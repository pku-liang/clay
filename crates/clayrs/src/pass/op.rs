use cmt2::cmtir::TypedLit;

use crate::ast::{BinaryOp, Flow, UnaryOp};
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OpIdx(pub usize);

impl Display for OpIdx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}


#[derive(Debug, Clone)]
pub enum Op {
    Lit(TypedLit),
    // Tuple(Vec<OpIdx>),   // for nested tuple, no effects
    ExtRef(String),  // ref to input or local/global state
    Unary(UnaryOp, OpIdx),             // immediate
    Binary(BinaryOp, OpIdx, OpIdx),    // immediate
    // Repeat(u32, OpIdx),                // immediate
    Index(OpIdx, OpIdx),               // immediate
    Slice(OpIdx, OpIdx, OpIdx),        // immediate
    Concat(Vec<OpIdx>),                // immediate
    Match(OpIdx, Vec<(OpIdx, OpIdx)>), // immediate
    IfElse(OpIdx, OpIdx, OpIdx),       // immediate

    RegReadReq(OpIdx),              // need wait ready
    RegReadResp(OpIdx),             // need wait valid
    RegWriteReq(OpIdx, OpIdx),      // need wait ready

    MemReadReq(OpIdx),              // need wait ready
    MemReadResp(OpIdx),             // need wait valid

    // addr, data, mask
    // need wait ready
    MemWriteReq(OpIdx, OpIdx, OpIdx),

    StateRead(String),            // direct local register read
    StateWrite(String, OpIdx),    // direct local register write

    // condition, pc
    PcUpdate(OpIdx, OpIdx)          // PcUpdate
}

impl Op {
    pub fn predecessors(&self) -> Vec<OpIdx> {
        match self {
            Op::Lit(_) => vec![],
            Op::ExtRef(_) => vec![],
            Op::Unary(_, op_idx) => vec![*op_idx],
            Op::Binary(_, op_idx, op_idx1) => vec![*op_idx, *op_idx1],
            Op::Index(op_idx, op_idx1) => vec![*op_idx, *op_idx1],
            Op::Slice(op_idx, op_idx1, op_idx2) => vec![*op_idx, *op_idx1, *op_idx2],
            Op::Concat(vec) => vec.clone(),
            Op::Match(op_idx, vec) => {
                let mut preds = vec![*op_idx];
                vec.iter().for_each(|(op_idx1, op_idx2)| {
                    preds.push(*op_idx1);
                    preds.push(*op_idx2);
                });
                preds
            },
            Op::IfElse(op_idx, op_idx1, op_idx2) => vec![*op_idx, *op_idx1, *op_idx2],
            Op::RegReadReq(op_idx) => vec![*op_idx],
            Op::RegReadResp(op_idx) => vec![*op_idx],
            Op::RegWriteReq(op_idx, op_idx1) => vec![*op_idx, *op_idx1],
            Op::MemReadReq(op_idx) => vec![*op_idx],
            Op::MemReadResp(op_idx) => vec![*op_idx],
            Op::MemWriteReq(op_idx, op_idx1, op_idx2) => vec![*op_idx, *op_idx1, *op_idx2],
            Op::StateRead(_) => vec![],
            Op::StateWrite(_, op_idx) => vec![*op_idx],
            Op::PcUpdate(op_idx, op_idx1) => vec![*op_idx, *op_idx1],
        }
    }
}