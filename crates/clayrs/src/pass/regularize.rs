use crate::pass::cdfg::{BasicBlock, CDFG};
use crate::pass::op::{Op, OpIdx};
use crate::pass::sched::{ModuloSchedule, Schedule, ScheduledAt, SequentialSchedule};
use crate::pass::type_infer::TypeInferContext;
use cmt2::cmtir::Type;
use std::collections::HashMap;

use super::*;

#[derive(Debug, Clone)]
pub enum BufferAlloc {
    /// Stage buffer for modulo scheduling (buffer instance name, type)
    StageBuffer(String, Type, usize),
    /// Register for sequential scheduling (register instance name, type)
    Register(String, Type),
    /// No buffer needed (literal can be inlined)
    Inline,
}

#[derive(Debug, Clone)]
pub struct RegularizedOp {
    /// Original operation
    pub op: Op,
    /// Buffer allocation for this operation's output
    pub buffer: BufferAlloc,
    /// Stage where this operation is executed
    pub stage: usize,
    /// Type of this operation
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct RegularizedBB {
    /// Original basic block index
    pub bb_idx: usize,
    /// Regularized operations
    pub ops: Vec<RegularizedOp>,
    /// Schedule information
    pub schedule: Schedule,
    /// Mapping from original op_idx to regularized op_idx
    pub op_map: HashMap<usize, usize>,
}

impl RegularizedBB {
    pub fn get_n_stages(&self) -> usize {
        match &self.schedule {
            Schedule::Modulo(mod_sched) => mod_sched.latency,
            Schedule::Sequential(seq_sched) => seq_sched.latency,
        }
    }
}

pub struct RegularizationContext {
    /// Type inference context
    type_ctx: TypeInferContext,
    /// Counter for generating unique buffer names
    buffer_counter: usize,
}

impl RegularizationContext {
    pub fn new() -> Self {
        Self {
            type_ctx: TypeInferContext::new(),
            buffer_counter: 0,
        }
    }

    fn gen_buffer_name(&mut self, prefix: &str) -> String {
        let name = format!("__{}_{}", prefix, self.buffer_counter);
        self.buffer_counter += 1;
        name
    }

    /// Regularize a basic block with its schedule
    pub fn regularize_bb(
        &mut self,
        cdfg: &CDFG,
        bb_idx: usize,
        schedule: &Schedule,
    ) -> RegularizedBB {
        let bb = &cdfg.blocks[bb_idx];
        let mut ops = Vec::new();
        let mut op_map = HashMap::new();

        match schedule {
            Schedule::Sequential(seq_sched) => {
                self.regularize_sequential(cdfg, bb_idx, bb, seq_sched, &mut ops, &mut op_map)
            }
            Schedule::Modulo(mod_sched) => {
                self.regularize_modulo(cdfg, bb_idx, bb, mod_sched, &mut ops, &mut op_map)
            }
        }

        RegularizedBB {
            bb_idx,
            ops,
            schedule: schedule.clone(),
            op_map,
        }
    }

    fn regularize_sequential(
        &mut self,
        cdfg: &CDFG,
        bb_idx: usize,
        bb: &BasicBlock,
        schedule: &SequentialSchedule,
        ops: &mut Vec<RegularizedOp>,
        op_map: &mut HashMap<usize, usize>,
    ) {
        for (op_idx, op) in bb.ops.iter().enumerate() {
            let ty = self.type_ctx.infer_type(cdfg, bb_idx, OpIdx(op_idx));
            let stage = schedule.schedule[op_idx].stage().unwrap();

            let buffer = if self.needs_buffer(op, bb, op_idx, schedule) {
                // Allocate a register for sequential scheduling
                let name = self.gen_buffer_name(format!("reg_bb_{}", bb_idx).as_str());
                BufferAlloc::Register(name, ty.clone())
            } else if matches!(op, Op::Lit(_)) {
                // Literals can be inlined
                BufferAlloc::Inline
            } else {
                // No buffer needed (used in same stage or special ops)
                BufferAlloc::Inline
            };

            let reg_op = RegularizedOp {
                op: op.clone(),
                buffer,
                stage,
                ty,
            };

            op_map.insert(op_idx, ops.len());
            ops.push(reg_op);
        }
    }

    fn regularize_modulo(
        &mut self,
        cdfg: &CDFG,
        bb_idx: usize,
        bb: &BasicBlock,
        schedule: &ModuloSchedule,
        ops: &mut Vec<RegularizedOp>,
        op_map: &mut HashMap<usize, usize>,
    ) {
        for (op_idx, op) in bb.ops.iter().enumerate() {
            let ty = self.type_ctx.infer_type(cdfg, bb_idx, OpIdx(op_idx));
            let stage = schedule.schedule[op_idx].stage().unwrap();

            let buffer = if let Some(stage) = self.needs_modulo_buffer(op, bb, op_idx, schedule) {
                // Allocate a stage buffer for modulo scheduling
                let name = self.gen_buffer_name(format!("t_bb_{}", bb_idx).as_str());
                BufferAlloc::StageBuffer(name, ty.clone(), stage)
            } else if matches!(op, Op::Lit(_)) {
                // Literals can be inlined
                BufferAlloc::Inline
            } else {
                // No buffer needed
                BufferAlloc::Inline
            };

            let reg_op = RegularizedOp {
                op: op.clone(),
                buffer,
                stage,
                ty,
            };

            op_map.insert(op_idx, ops.len());
            ops.push(reg_op);
        }
    }

    /// Check if a buffer is needed for this operation in sequential scheduling
    fn needs_buffer(
        &self,
        op: &Op,
        bb: &BasicBlock,
        op_idx: usize,
        schedule: &SequentialSchedule,
    ) -> bool {
        // Memory/Reg requests don't produce data that needs buffering
        if matches!(
            op,
            Op::MemReadReq(_) | Op::MemWriteReq(_, _, _) | 
            Op::RegReadReq(_) | Op::RegWriteReq(_, _) | 
            Op::StateWrite(_, _) | Op::PcUpdate(_, _)
        ) {
            return false;
        }

        // Check if any consumer is in a different stage
        let my_stage = schedule.schedule[op_idx].stage().unwrap();
        
        for (other_idx, other_op) in bb.ops.iter().enumerate() {
            if other_idx == op_idx {
                continue;
            }
            
            let preds = other_op.predecessors();
            if preds.contains(&OpIdx(op_idx)) {
                let other_stage = schedule.schedule[other_idx].stage().unwrap();
                if other_stage != my_stage {
                    return true;
                }
            }
        }

        false
    }

    /// Check if a buffer is needed for this operation in modulo scheduling
    fn needs_modulo_buffer(
        &self,
        op: &Op,
        bb: &BasicBlock,
        op_idx: usize,
        schedule: &ModuloSchedule,
    ) -> Option<usize> {
        // Memory/Reg requests don't produce data that needs buffering
        if matches!(
            op,
            Op::MemReadReq(_) | Op::MemWriteReq(_, _, _) | 
            Op::RegReadReq(_) | Op::RegWriteReq(_, _) | 
            Op::StateWrite(_, _) | Op::PcUpdate(_, _)
        ) {
            return None;
        }

        // In modulo scheduling, we need buffers for most operations that cross stages
        let my_stage = schedule.schedule[op_idx].stage().unwrap();
        let n_stages = schedule.latency - 1;

        // cond will be access at II-1 and last
        if Some(OpIdx(op_idx)) == bb.cond {
            return if my_stage == n_stages {
                None
            } else {
                Some(n_stages)
            };
        }

        let mut ret: Option<usize> = None;
        
        for (other_idx, other_op) in bb.ops.iter().enumerate() {
            if other_idx == op_idx {
                continue;
            }
            
            let preds = other_op.predecessors();
            if preds.contains(&OpIdx(op_idx)) {
                let other_stage = schedule.schedule[other_idx].stage().unwrap();
                // In modulo scheduling, different stages always need buffers
                if other_stage != my_stage {
                    ret = match ret {
                        Some(r) => Some(r.max(other_stage)),
                        None => Some(other_stage),
                    };
                }
            }
        }

        ret
    }

    /// Get the type inference context
    pub fn type_context(&self) -> &TypeInferContext {
        &self.type_ctx
    }

    /// Get mutable type inference context
    pub fn type_context_mut(&mut self) -> &mut TypeInferContext {
        &mut self.type_ctx
    }
}

/// Regularize all basic blocks in a CDFG
pub fn regularize_cdfg(
    cdfg: &CDFG,
    schedules: &[Schedule],
) -> (Vec<RegularizedBB>, TypeInferContext) {
    let mut ctx = RegularizationContext::new();
    let mut result = Vec::new();

    for (bb_idx, schedule) in schedules.iter().enumerate() {
        let reg_bb = ctx.regularize_bb(cdfg, bb_idx, schedule);
        result.push(reg_bb);
    }

    (result, ctx.type_ctx)
}
