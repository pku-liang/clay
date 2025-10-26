use std::result;

/// Entry point for the Clay compiler.
use clap::Parser;
use cmt2::cmtc::{elaborate, sv_config, utils};
use miette::IntoDiagnostic;

use clayrs::{backends::rocc::{make_rocc_module}, pass::{cdfg::{BlockIdx, CDFG}, sched::{self, modulo_sched_bb, sched_bb, ClayCOPBackend, ClaySimpleBackend, RoCCCOPBackend, Schedule, MAX_STAGES}}};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long, default_value_t = String::from("samples/stream_add.cadl"))]
    input_file: String,
}

fn main() -> miette::Result<()>{
    let args = Args::parse();
    println!("Input file: {}", args.input_file);

    let input = std::fs::read_to_string(&args.input_file)
        .into_diagnostic()?;

    let result = clayrs::ast::parser::parse_proc(&input, Some(args.input_file.clone()))?;

    let mut cdfg = CDFG::new();
    for (_, flow) in result.flows() {
        cdfg.add_flow(flow);
    }

    cdfg.regularize();

    let backend = &RoCCCOPBackend;
    let mut schedules = vec![];
    for (idx, bb) in cdfg.blocks.iter().enumerate() {
        if let Some(BlockIdx(true_branch)) = bb.true_branch {
            if true_branch == idx {
                // inner most loop
                // let schedule = modulo_sched_bb(bb, 2, 10, backend).unwrap();
                for ii in 1..MAX_STAGES {
                    if let Some(schedule) = modulo_sched_bb(bb, ii as usize, MAX_STAGES as usize, backend) {
                        println!("Found schedule for BB {} with II={}", idx, ii);
                        schedules.push(Schedule::Modulo(schedule));
                        break;
                    }
                }
                continue;
            }
        }

        let schedule = sched_bb(bb, MAX_STAGES as usize, backend).unwrap();
        schedules.push(Schedule::Sequential(schedule));
        println!("Found sequential schedule for BB {}", idx);
    }

    // utils::setup_logger();
    let m = make_rocc_module(&mut cdfg, &schedules);
    elaborate(m, sv_config("rocc_top.sv")).unwrap();

    println!("Custom RoCC module generated successfully at {}.", "rocc_top.sv");
    Ok(())
}