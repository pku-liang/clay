use crate::pass::{cdfg::{BasicBlock, CDFG}, op::{Op, OpIdx}};
use good_lp::{self, constraint, default_solver, solvers::coin_cbc::CoinCbcProblem, variable, SolverModel};
use good_lp::Solution;

pub const MAX_STAGES: u32 = 20;

use super::*;


#[derive(Debug, Clone)]
pub enum ScheduledAt {
    Any,
    Stage(usize),
}

impl ScheduledAt {
    pub fn stage(&self) -> Option<usize> {
        match self {
            ScheduledAt::Any => None,
            ScheduledAt::Stage(s) => Some(*s),
        }
    }
}


#[derive(Debug, Clone)]
pub struct SequentialSchedule {
    pub schedule: Vec<ScheduledAt>,
    pub latency: usize
}

#[derive(Debug, Clone)]
pub struct ModuloSchedule {
    pub schedule: Vec<ScheduledAt>,
    pub II: usize,
    pub latency: usize
}

#[derive(Debug, Clone)]
pub enum Schedule {
    Sequential(SequentialSchedule),
    Modulo(ModuloSchedule)
}

// impl Schedule {
//     fn ii(&self) -> usize {
//         match self {
//             Schedule::Sequential(schedule) => schedule.II,
//             _ => panic!()
//         }
//     }

//     fn is_modulo(&self) -> bool {
//         return matches!(self, Schedule::Modulo(_))
//     }

//     fn is_sequential(&self) -> bool {
//         return matches!(self, Schedule::Sequential(_))
//     }

//     fn ref_mut(&mut self) -> &mut Vec<ScheduledAt> {
//         match self {
//             Schedule::Sequential(sequential_schedule) => &mut sequential_schedule.schedule,
//             Schedule::Modulo(modulo_schedule) => &mut modulo_schedule.schedule
//         }
//     }
// }


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarKey {
    OpAtStage(usize, usize), // op idx, stage
    StageOfOp(usize),        // stage
    LifeTimeOfOp(usize),     // op idx
    OverallLatency,
    OverallLifetime
}

struct VarSet {
    vars: good_lp::ProblemVariables,
    var_map: HashMap<VarKey, good_lp::Variable>,
}

// seq
fn get_vars(bb: &BasicBlock, max_stages: usize) -> VarSet {
    let mut vars = good_lp::variables!();
    let mut var_map = HashMap::new();

    for (idx, _) in bb.ops.iter().enumerate() {
        for stage in 0..max_stages {
            let var = vars.add(variable().binary().name(&format!("x{}_s{}", idx, stage)));
            var_map.insert(VarKey::OpAtStage(idx, stage), var);
        }

        let var = vars.add(variable().integer().min(0).max(max_stages as i32 - 1).name(&format!("stage_of_{}", idx)));
        var_map.insert(VarKey::StageOfOp(idx), var);
        let var = vars.add(variable().integer().min(0).max(max_stages as i32).name(&format!("lifetime_of_{}", idx)));
        var_map.insert(VarKey::LifeTimeOfOp(idx), var);
    }

    let var = vars.add(variable().integer().min(0).max(max_stages as i32).name("overall_latency"));
    var_map.insert(VarKey::OverallLatency, var);

    let var = vars.add(variable().integer().min(0).max(max_stages as i32).name("overall_latency"));
    var_map.insert(VarKey::OverallLifetime, var);

    return VarSet { vars, var_map };
}

fn get_op_latency(op: &Op) -> u32 {
    match op {
        Op::RegReadReq(_) | Op::MemReadReq(_) => 1,
        _ => 0,
    }
}

fn build_constraints(bb: &BasicBlock, vars: good_lp::ProblemVariables, var_map: &HashMap<VarKey, good_lp::Variable>, max_stages: usize) -> CoinCbcProblem {
    let overall_latency_var = var_map.get(&VarKey::OverallLatency).unwrap();
    let overall_lifetime_var = var_map.get(&VarKey::OverallLifetime).unwrap();

    // A * Overall_Latency + B * LifeTime Cost (registers)
    // Currently set B = 0 for best latency
    let mut problem = vars.minimise(99999 * *overall_latency_var + overall_lifetime_var).using(default_solver);
    problem.set_parameter("loglevel", "0");

    let mut life_sum = good_lp::Expression::from(0);

    for (idx, op) in bb.ops.iter().enumerate() {
        // check predecessors
        life_sum = life_sum + var_map.get(&VarKey::LifeTimeOfOp(idx)).unwrap();
        let preds = op.predecessors();
        let op_stage_var = var_map.get(&VarKey::StageOfOp(idx)).unwrap();
        for pred in preds.iter() {
            // latency constraint & dependency
            let latency = get_op_latency(&bb.ops[pred.0]);
            let pred_stage_var = var_map.get(&VarKey::StageOfOp(pred.0)).unwrap();
            problem.add_constraint(constraint!(*pred_stage_var + latency <= op_stage_var));

            // lifetime
            let pred_life_var = var_map.get(&VarKey::LifeTimeOfOp(pred.0)).unwrap();
            problem.add_constraint(constraint!(*pred_life_var + *pred_stage_var + latency >= *op_stage_var));
        }
        // overall latency constraint
        problem.add_constraint(constraint!(*op_stage_var + get_op_latency(op) <= *overall_latency_var));

        // one-hot constraint
        let mut one_hot_expr = good_lp::Expression::from(0);
        let mut stage_expr = good_lp::Expression::from(0);
        for stage in 0..max_stages {
            let stage_var = var_map.get(&VarKey::OpAtStage(idx, stage)).unwrap();
            one_hot_expr = one_hot_expr + stage_var;
            stage_expr = stage_expr + *stage_var * (stage as i32);
        }
        problem.add_constraint(constraint!(one_hot_expr == 1));
        problem.add_constraint(constraint!(stage_expr == *op_stage_var));

        if matches!(op, Op::ExtRef(_)) {
            problem.add_constraint(constraint!(*op_stage_var == 0));
        }
    }

    return problem;
}

pub fn sched_bb(bb: &BasicBlock, max_stages: usize, backend: &impl Backend) -> Option<SequentialSchedule> {
    let var_set = get_vars(bb, max_stages);
    let mut problem = build_constraints(bb, var_set.vars, &var_set.var_map, max_stages);

    backend.add_constraints(&mut problem, &var_set.var_map, bb, max_stages);

    // todo: add resource constraints

    let solution = problem.solve().ok()?;

    let mut schedule = vec![];
    let mut latency = 0;
    for (idx, _) in bb.ops.iter().enumerate() {
        let stage_var = var_set.var_map.get(&VarKey::StageOfOp(idx)).unwrap();
        let stage = solution.value(*stage_var) as usize;
        schedule.push(ScheduledAt::Stage(stage));

        if stage > latency {
            latency = stage;
        }
    }

    Some(SequentialSchedule { schedule, latency: latency + 1 })
}

pub fn modulo_sched_bb(bb: &BasicBlock, II: usize, max_stages: usize, backend: &impl Backend) -> Option<ModuloSchedule> {
    let var_set = get_vars(bb, max_stages);

    let mut problem = build_constraints(bb, var_set.vars, &var_set.var_map, max_stages);

    backend.add_constraints_modulo(&mut problem, &var_set.var_map, bb, II, max_stages);

    if let Some(cond) = bb.cond {
        let cond_stage = var_set.var_map.get(&VarKey::StageOfOp(cond.0)).unwrap();
        problem.add_constraint(constraint!(*cond_stage <= II as u32 - 1));
    }

    let solution = problem.solve().ok()?;
    let mut schedule = vec![];
    let mut latency = 0;
    for (idx, _) in bb.ops.iter().enumerate() {
        let stage_var = var_set.var_map.get(&VarKey::StageOfOp(idx)).unwrap();
        let stage = solution.value(*stage_var) as usize;
        schedule.push(ScheduledAt::Stage(stage));
        if stage > latency {
            latency = stage;
        }
    }

    Some(ModuloSchedule { schedule, II, latency: latency + 1 })
}


pub trait Backend {
    fn sanity_check(&self, cdfg: &CDFG) -> bool;

    fn stat_cdfg(&self, cdfg: &CDFG) -> HashMap<String, usize> {
        let mut bins = HashMap::new();
        cdfg.blocks.iter().map(|block| block.ops.iter()).flatten().for_each(|op| {
            let key = match op {
                op::Op::RegReadReq(op_idx) => Some("RegRead"),
                op::Op::RegWriteReq(op_idx, op_idx1) => Some("RegWrite"),
                op::Op::MemReadReq(op_idx) => Some("MemRead"),
                op::Op::MemWriteReq(op_idx, op_idx1, op_idx2) => Some("MemWrite"),
                op::Op::PcUpdate(op_idx, op_idx1) => Some("PcUpdate"),
                _ => None
            };
            if let Some(key) = key {
                let key = key.to_string();
                let cnt = bins.get(&key).unwrap_or(&0);
                bins.insert(key, *cnt+1);
            }
        });
        bins
    }

    fn dedicated_memory_port(&self) -> bool {
        true
    }

    fn resources(&self) -> HashMap<String, usize> {
        // default: no resource limits
        HashMap::from(
            [("RegReadReq".to_string(), 2),
             ("RegReadResp".to_string(), 2),
             ("RegWriteReq".to_string(), 1),
             ("MemReadReq".to_string(), 1),
             ("MemReadResp".to_string(), 1),
             ("MemWriteReq".to_string(), 1),
             ("PcUpdateReq".to_string(), 1)]
        )
    }

    fn add_constraints_modulo(&self, problem: &mut CoinCbcProblem, var_map: &HashMap<VarKey, good_lp::Variable>, bb: &BasicBlock, II: usize, max_stages: usize) {
        // default: no extra constraints
        let mut usage = HashMap::new();
        for (idx, op) in bb.ops.iter().enumerate() {
            for s in 0..max_stages {
                let var = var_map.get(&VarKey::OpAtStage(idx, s)).unwrap();
                let key = match op {
                    op::Op::RegReadReq(_) => "RegReadReq",
                    op::Op::RegReadResp(_) => "RegReadResp",
                    op::Op::RegWriteReq(_, _) => "RegWriteReq",
                    op::Op::MemReadReq(_) => if self.dedicated_memory_port() { "MemReadReq" } else { "Memory" },
                    op::Op::MemReadResp(_) => "MemReadResp",
                    op::Op::MemWriteReq(_, _, _) => if self.dedicated_memory_port() { "MemWriteReq" } else { "Memory" },
                    op::Op::PcUpdate(_, _) => "PcUpdateReq",
                    _ => continue
                }.to_string();
                let entry = usage.entry((key, s % II)).or_insert(vec![]);
                entry.push(*var);
            }
        }

        let resources = self.resources();
        for ((key, stage), vars) in usage.iter() {
            // at most one usage per stage
            let expr = vars.iter().fold(good_lp::Expression::from(0), |acc, var| acc + *var);
            problem.add_constraint(constraint!(expr <= resources[key] as u32));
        }

        // loop carried dependencies
        let mut loop_carried= HashMap::new();
        for (i, op) in bb.ops.iter().enumerate() {
            if let Op::StateRead(state) = op {
                if bb.defs.contains(state) {
                    let stage = var_map.get(&VarKey::StageOfOp(i)).unwrap();
                    loop_carried.insert(state.clone(), *stage);
                }
            } else if let Op::StateWrite(state, _) = op {
                if bb.uses.contains(state) {
                    let stage = var_map.get(&VarKey::StageOfOp(i)).unwrap();
                    let use_stage = loop_carried.get(state).unwrap();
                    problem.add_constraint(constraint!(*use_stage + II as u32 >= *stage + 1));
                }
            }
        }

    }


    fn add_constraints(&self, problem: &mut CoinCbcProblem, var_map: &HashMap<VarKey, good_lp::Variable>, bb: &BasicBlock, max_stages: usize) {
        // default: no extra constraints
        let mut usage = HashMap::new();
        for (idx, op) in bb.ops.iter().enumerate() {
            for s in 0..max_stages {
                let var = var_map.get(&VarKey::OpAtStage(idx, s)).unwrap();
                let key = match op {
                    op::Op::RegReadReq(_) => "RegReadReq",
                    op::Op::RegReadResp(_) => "RegReadResp",
                    op::Op::RegWriteReq(_, _) => "RegWriteReq",
                    op::Op::MemReadReq(_) => if self.dedicated_memory_port() { "MemReadReq" } else { "Memory" },
                    op::Op::MemReadResp(_) => "MemReadResp",
                    op::Op::MemWriteReq(_, _, _) => if self.dedicated_memory_port() { "MemWriteReq" } else { "Memory" },
                    op::Op::PcUpdate(_, _) => "PcUpdateReq",
                    _ => continue
                }.to_string();
                let entry = usage.entry((key, s)).or_insert(vec![]);
                entry.push(*var);
            }
        }

        let resources = self.resources();
        for ((key, stage), vars) in usage.iter() {
            // at most one usage per stage
            let expr = vars.iter().fold(good_lp::Expression::from(0), |acc, var| acc + *var);
            problem.add_constraint(constraint!(expr <= resources[key] as u32));
        }
    }
}


pub struct ClaySimpleBackend;
impl Backend for ClaySimpleBackend {
    fn sanity_check(&self, cdfg: &CDFG) -> bool {
        if cdfg.blocks.len() != 1 {
            return false;
        }

        let stat = self.stat_cdfg(cdfg);

        if stat["RegRead"] > 2 ||
           stat["RegWrite"] > 1 ||
           stat["MemRead"] > 0 ||
           stat["MemWrite"] > 0 ||
           stat["PcUpdate"] > 0 {
             return false;
           }

        true
    }
}

pub struct ClayInPipelineBackend;
impl Backend for ClayInPipelineBackend {
    fn sanity_check(&self, cdfg: &CDFG) -> bool {
        if cdfg.blocks.len() != 1 {
            return false;
        }

        let stat = self.stat_cdfg(cdfg);

        if stat["RegRead"] > 2 ||
           stat["RegWrite"] > 1 ||
           stat["MemRead"] + stat["MemWrite"] > 1 ||
           stat["PcUpdate"] > 1 {
             return false;
           }

        true
    }
}

pub struct ClayCOPBackend;
impl Backend for ClayCOPBackend {
    fn sanity_check(&self, cdfg: &CDFG) -> bool {
        let stat = self.stat_cdfg(cdfg);

        if stat["RegRead"] > 2 ||
           stat["RegWrite"] > 1 ||
           stat["PcUpdate"] > 0 {
             return false;
           }

        true
    }
}

pub struct RoCCCOPBackend;
impl Backend for RoCCCOPBackend {
    fn sanity_check(&self, cdfg: &CDFG) -> bool {
        let stat = self.stat_cdfg(cdfg);

        if stat["RegRead"] > 2 ||
           stat["RegWrite"] > 1 ||
           stat["PcUpdate"] > 0 {
             return false;
           }

        true
    }

    fn dedicated_memory_port(&self) -> bool {
        // no concurrent read and write
        false
    }
    fn resources(&self) -> HashMap<String, usize> {
        // default: no resource limits
        HashMap::from(
            [("RegReadReq".to_string(), 2),
             ("RegReadResp".to_string(), 2),
             ("RegWriteReq".to_string(), 1),
             ("Memory".to_string(), 1),
             ("MemReadResp".to_string(), 1),
             ("PcUpdateReq".to_string(), 1)]
        )
    }
}


struct ScheduleRegularize<'a> {
    bb: &'a mut BasicBlock,
    schedule: &'a mut Schedule,
    prefix: &'a str,
    origin_len: usize
}

// impl<'a> ScheduleRegularize<'a> {
//     fn new(bb: &'a mut BasicBlock, schedule: &'a mut Schedule, prefix: &'a str) -> Self {
//         let origin_len = bb.ops.len();
//         ScheduleRegularize {
//             bb,
//             schedule,
//             prefix,
//             origin_len
//         }
//     }

//     fn regularize(&mut self) {
//         match self.schedule {
//             Schedule::Sequential(sequential_schedule) => self.seq_regularize(),
//             Schedule::Modulo(modulo_schedule) => self.modulo_regularize()
//         }
//     }

//     fn push_op(&mut self, op: Op, stage: usize) -> OpIdx {
//         let ret = self.bb.add_op(op);
//         self.schedule.ref_mut().push(ScheduledAt::Stage(stage));
//         ret
//     }

//     fn seq_regularize(&mut self) {
//         let op_map = HashMap::new();

//         for i in 0..self.origin_len {
//             let cur_stage = self.schedule.ref_mut()[i].stage().unwrap();

//         }
//     }
// }

