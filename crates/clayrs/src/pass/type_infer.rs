use crate::ast::{BinaryOp, UnaryOp};
use crate::pass::cdfg::{BasicBlock, CDFG};
use crate::pass::op::{Op, OpIdx};
use cmt2::cmtir::Type;
use std::collections::HashMap;

use super::*;

/// Type inference cache for operations
pub struct TypeInferContext {
    /// Cache of inferred types for each (bb_idx, op_idx)
    cache: HashMap<(usize, usize), Type>,
    name_in_flight: Set<String>
}

impl TypeInferContext {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            name_in_flight: Set::new(),
        }
    }

    /// Infer the type of an operation in a basic block
    pub fn infer_type(&mut self, cdfg: &CDFG, bb_idx: usize, op_idx: OpIdx) -> Type {
        let key = (bb_idx, op_idx.0);
        if let Some(ty) = self.cache.get(&key) {
            return ty.clone();
        }

        let bb = &cdfg.blocks[bb_idx];
        let op = &bb.ops[op_idx.0];
        let ty = self.infer_op_type(cdfg, bb_idx, bb, op);
        
        self.cache.insert(key, ty.clone());
        ty
    }

    fn infer_op_type(&mut self, cdfg: &CDFG, bb_idx: usize, bb: &BasicBlock, op: &Op) -> Type {
        match op {
            Op::Lit(lit) => lit.ty.clone(),
            
            Op::ExtRef(name) => {
                // ExtRef should have been regularized to StateRead
                // But if still present, check inputs or find StateWrite
                if bb.inputs.contains(name) {
                    // Default to 32-bit for inputs (rs1, rs2, etc.)
                    match &name[..] {
                        "rs1" | "rs2" => Type::UInt(32),
                        "funct3" => Type::UInt(3),
                        "funct7" => Type::UInt(7),
                        x => {
                            println!("Warning: Unknown ExtRef input '{}', defaulting to 32-bit", x);
                            Type::UInt(32)
                        }
                    }
                } else {
                    panic!("ExtRef '{}' not in inputs; cannot infer type", name);
                }
            }
            
            Op::StateRead(name) => {
                self.find_state_write_type(cdfg, name)
            }
            
            Op::StateWrite(_, data_idx) => {
                // StateWrite has no output type, but we infer the data type
                Type::UInt(0)
            }
            
            Op::Unary(unary_op, operand) => {
                let operand_ty = self.infer_type(cdfg, bb_idx, *operand);
                // match unary_op {
                //     UnaryOp::Not | UnaryOp::Neg | UnaryOp::SignedCast | 
                //     UnaryOp::UnsignedCast | UnaryOp::BitNot => operand_ty,
                // }
                operand_ty
            }
            
            Op::Binary(binary_op, lhs, rhs) => {
                let lhs_ty = self.infer_type(cdfg, bb_idx, *lhs);
                let rhs_ty = self.infer_type(cdfg, bb_idx, *rhs);
                
                match binary_op {
                    // Comparison operations return 1-bit boolean
                    BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | 
                    BinaryOp::Gt | BinaryOp::Ge => Type::UInt(1),
                    
                    // Arithmetic and logical operations preserve width
                    _ => {
                        if let Type::UInt(lhs_w) = lhs_ty {
                            if let Type::UInt(rhs_w) = rhs_ty {
                                Type::UInt(std::cmp::max(lhs_w, rhs_w))
                            } else {
                                lhs_ty
                            }
                        } else {
                            lhs_ty
                        }
                    }
                }
            }
            
            Op::Index(array, _) => {
                // Indexing returns element type - assume 1-bit for simplicity
                Type::UInt(1)
            }
            
            Op::Slice(expr, hi, lo) => {
                // Slice width is determined by hi and lo
                if let Op::Lit(hi_lit) = &bb.ops[hi.0] {
                    if let Op::Lit(lo_lit) = &bb.ops[lo.0] {
                        let hi_val = u64::try_from(hi_lit.value()).unwrap_or(31);
                        let lo_val = u64::try_from(lo_lit.value()).unwrap_or(0);
                        let width = (hi_val - lo_val + 1) as u32;
                        return Type::UInt(width);
                    }
                }
                // Default if can't determine
                Type::UInt(0)
            }
            
            Op::Concat(operands) => {
                let mut total_width = 0;
                for op_idx in operands {
                    let ty = self.infer_type(cdfg, bb_idx, *op_idx);
                    if let Type::UInt(w) = ty {
                        total_width += w;
                    }
                }
                Type::UInt(total_width)
            }
            
            Op::Match(_, branches) => {
                // Return type of first branch value
                // if let Some((_, val_idx)) = branches.first() {
                //     self.infer_type(cdfg, bb_idx, *val_idx)
                // } else {
                //     Type::UInt(32)
                // }

                for (_, val_idx) in branches {
                    let ty = self.infer_type(cdfg, bb_idx, *val_idx);
                    if ty != Type::UInt(0) {
                        return ty;
                    }
                }
                Type::UInt(0)
            }
            
            Op::IfElse(_, then_idx, else_idx) => {
                let then_type = self.infer_type(cdfg, bb_idx, *then_idx);
                if then_type == Type::UInt(0) {
                    self.infer_type(cdfg, bb_idx, *else_idx)
                } else {
                    then_type
                }
            }
            
            Op::RegReadReq(_) => Type::UInt(0), // Request has no data
            Op::RegReadResp(_) => Type::UInt(32), // Response returns 32-bit data
            Op::RegWriteReq(_, _) => Type::UInt(0), // Write req has no output
            
            Op::MemReadReq(_) => Type::UInt(0), // Request has no data
            Op::MemReadResp(_) => Type::UInt(32), // Response returns 32-bit data
            Op::MemWriteReq(_, _, _) => Type::UInt(0), // Write req has no output
            
            Op::PcUpdate(_, _) => Type::UInt(0), // No output
        }
    }

    /// Find the type of a state variable by searching for StateWrite
    pub fn find_state_write_type(&mut self, cdfg: &CDFG, name: &str) -> Type {
        if self.name_in_flight.contains(name) {
            println!("Warning: Recursive type inference for '{}', defaulting to 0-bit", name);
            return Type::UInt(0);
        }

        // Special handling for known inputs
        self.name_in_flight.insert(name.to_string());

        match name {
            "rs1" | "rs2" | "rd" => return Type::UInt(32),
            _ => {}
        }

        // Search all basic blocks for StateWrite to this variable
        for (bb_idx, bb) in cdfg.blocks.iter().enumerate() {
            if bb.defs.contains(name) {
                for (op_idx, op) in bb.ops.iter().enumerate() {
                    if let Op::StateWrite(state_name, data_idx) = op {
                        if state_name == name {
                            let ty = self.infer_type(cdfg, bb_idx, *data_idx);
                            self.name_in_flight.remove(name);
                            return ty;
                        }
                    }
                }
            }
        }

        self.name_in_flight.remove(name);
        // Default to 32-bit if not found
        Type::UInt(0)
    }

    /// Get cached type if available
    pub fn get_cached(&self, bb_idx: usize, op_idx: OpIdx) -> Option<&Type> {
        self.cache.get(&(bb_idx, op_idx.0))
    }
}
