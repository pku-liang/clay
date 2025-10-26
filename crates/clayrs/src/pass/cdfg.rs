use crate::ast::{expr, AssignKind, Expr, Flow, Stmt, UnaryOp};

use super::*;
use cmt2::cmtir::{RadixIntLit, Type, TypedLit};
use op::*;
use proc_macro2::extra;

fn mask_f() -> TypedLit {
    TypedLit::new( RadixIntLit::from(0xfu8), Type::UInt(4).into())
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub inputs: Vec<String>, // type?
    pub ops: Vec<Op>,
    pub cond: Option<OpIdx>, // condition for branching
    pub true_branch: Option<BlockIdx>,
    pub false_branch: Option<BlockIdx>,
    pub defs: Set<String>,
    pub uses: Set<String>,
    pub var_map: HashMap<String, OpIdx>
}


#[derive(Debug, Clone)]
pub struct CDFG {
    pub blocks: Vec<BasicBlock>,
    pub vars: Set<String>
}

#[derive(Debug, Clone, Copy)]
pub struct BlockIdx(pub usize);

impl BasicBlock {
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            ops: Vec::new(),
            cond: None,
            true_branch: None,
            false_branch: None,
            defs: Set::new(),
            uses: Set::new(),
            var_map: HashMap::new()
        }
    }
    pub fn add_op(&mut self, op: Op) -> OpIdx {
        let idx = self.ops.len();
        self.ops.push(op);
        OpIdx(idx)
    }
}


impl CDFG {
    pub fn new() -> Self {
        Self { blocks: Vec::new(), vars: Set::new() }
    }

    pub fn add_block(&mut self, block: BasicBlock) -> BlockIdx {
        let idx = self.blocks.len();
        self.blocks.push(block);
        BlockIdx(idx)
    }

    pub fn current_block_mut(&mut self) -> &mut BasicBlock {
        self.blocks.last_mut().expect("No blocks in CDFG")
    }
}

impl CDFG {
    pub fn add_flow(&mut self, flow: &Flow) {
        let mut block = BasicBlock::new();
        for input in &flow.inputs {
            block.inputs.push(input.name().clone());
        }
        self.add_block(block);
        for stmt in flow.body.as_ref().unwrap_or(&vec![]) {
            self.add_stmt(stmt);
        }
    }

    pub fn add_op(&mut self, op: Op) -> OpIdx {
        self.current_block_mut().add_op(op)
    }

    // eval a rvalue
    pub fn add_expr(&mut self, expr: &Expr) -> OpIdx {
        // let bb = self.current_block_mut();
        match expr {
            Expr::Lit(typed_lit) => self.add_op(Op::Lit(typed_lit.clone())),
            Expr::Ident(name) => {
                if let Some(v) = self.current_block_mut().var_map.get(name) {
                    v.clone()
                } else {
                    let v = self.add_op(Op::ExtRef(name.clone()));
                    self.current_block_mut().var_map.insert(name.clone(), v);
                    self.current_block_mut().uses.insert(name.clone());
                    v
                }
            },
            Expr::Tuple(v) => {
                unimplemented!()
                // let ids = v.iter().map(|e| self.add_expr(e)).collect();
                // self.add_op(Op::Tuple(ids))
            },
            Expr::Binary(binary_op, lhs, rhs) => {
                let lhs = self.add_expr(lhs);
                let rhs = self.add_expr(rhs);
                self.add_op(Op::Binary(binary_op.clone(), lhs, rhs))
            }
            Expr::Unary(unary_op, expr) => {
                let expr = self.add_expr(expr);
                self.add_op(Op::Unary(unary_op.clone(), expr))
            }
            Expr::Call(fname, v) => {
                match &fname[..] {
                    "MemRead" => {
                        assert_eq!(v.len(), 1);
                        let addr = self.add_expr(&v[0]);
                        let req = self.add_op(Op::MemReadReq(addr));
                        let resp = self.add_op(Op::MemReadResp(req));
                        resp
                    },
                    "MemWrite" => {
                        assert!(v.len() <= 3);
                        assert!(v.len() >= 2);
                        let addr = self.add_expr(&v[0]);
                        let data = self.add_expr(&v[1]);
                        let mask = if v.len() == 3 {self.add_expr(&v[2])} else {self.add_op(Op::Lit(mask_f()))};
                        self.add_op(Op::MemWriteReq(addr, data, mask))
                    },
                    "PcUpdate" => {
                        assert!(v.len() == 2);
                        let cond = self.add_expr(&v[0]);
                        let pc = self.add_expr(&v[1]);
                        self.add_op(Op::PcUpdate(cond, pc))
                    },
                    "$signed" => {
                        assert!(v.len() == 1);
                        let expr = self.add_expr(&v[0]);
                        self.add_op(Op::Unary(UnaryOp::SignedCast, expr))
                    },
                    "$unsigned" => {
                        assert!(v.len() == 1);
                        let expr = self.add_expr(&v[0]);
                        self.add_op(Op::Unary(UnaryOp::UnsignedCast, expr))
                    },
                    x => {
                        panic!("unknown function call: {}", x);
                    }
                }
            }
            Expr::Repeat(n, expr) => {
                let expr = self.add_expr(expr);
                self.add_op(Op::Concat((0..*n).map(|_| expr).collect()))
            },
            Expr::Index(var, idx) => {
                let var = self.add_expr(var);
                let idx = self.add_expr(idx);
                self.add_op(Op::Index(var, idx))
            },
            Expr::Slice(expr, hb, lb) => {
                let expr = self.add_expr(expr);
                let hb = self.add_expr(hb);
                let lb = self.add_expr(lb);
                self.add_op(Op::Slice(expr, hb, lb))
            },
            Expr::Concat(v) => {
                let ids = v.iter().map(|e| self.add_expr(e)).collect();
                self.add_op(Op::Concat(ids))
            },
            Expr::Match(expr, pairs) => {
                // last as default
                let expr = self.add_expr(expr);
                let pairs = pairs.iter().map(
                    |(pattern, value)| {
                        let pattern = self.add_expr(pattern);
                        let value = self.add_expr(value);
                        (pattern, value)
                    }
                ).collect();
                self.add_op(Op::Match(expr, pairs))
            },
            Expr::If(expr, then_br, else_br) => {
                let expr = self.add_expr(expr);
                let then_br = self.add_expr(then_br);
                let else_br = self.add_expr(else_br);
                self.add_op(Op::IfElse(expr, then_br, else_br))
            },
            Expr::Memory(expr) | Expr::MemRead(expr) => {
                let addr = self.add_expr(expr);
                let req = self.add_op(Op::MemReadReq(addr));
                let resp = self.add_op(Op::MemReadResp(req));
                resp
            },
            Expr::MemWrite(expr, expr1, expr2) => {
                let addr = self.add_expr(expr);
                let data = self.add_expr(expr1);
                let mask = expr2.as_ref().map(|e| self.add_expr(&e)).unwrap_or(self.add_op(Op::Lit(mask_f())));
                self.add_op(Op::MemWriteReq(addr, data, mask))
            },
        }
    }

    fn extract_exprs(&mut self, expr: &Expr) -> Vec<OpIdx> {
        match expr {
            Expr::Tuple(v) => v.iter().map(|e| {self.add_expr(e)}).collect(),
            x => vec![self.add_expr(x)]
        }
    }

    pub fn add_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.add_stmt(stmt);
        }
    }

    // process stmts
    pub fn add_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assign(assign_kind, lhs, rhs) => {
                // if matches!(lhs.as_ref(), Expr::Memory(addr)) {
                if let Expr::Memory(addr) = lhs.as_ref() {
                    let addr = self.add_expr(addr);
                    let rhs = self.add_expr(rhs);
                    let mask = self.add_op(Op::Lit(mask_f()));
                    self.add_op(Op::MemWriteReq(addr, rhs, mask));
                } else {
                    // names
                    fn extract_names(expr: &Expr) -> Vec<String> {
                        match expr {
                            Expr::Ident(name) => vec![name.clone()],
                            Expr::Tuple(v) => v.iter().map(|e| extract_names(e)).flatten().collect(),
                            x => panic!("invalid lvalue: {}", x)
                        }
                    }

                    let names = extract_names(lhs);
                    let exprs = self.extract_exprs(rhs);
                    assert_eq!(names.len(), exprs.len());

                    for (name, expr) in names.iter().zip(exprs) {
                        self.current_block_mut().defs.insert(name.clone());
                        self.current_block_mut().var_map.insert(name.clone(), expr);

                        if matches!(assign_kind, AssignKind::Var) {
                            self.vars.insert(name.clone());
                        }
                    }
                }
            },
            Stmt::Expr(expr) => { self.add_expr(expr); }
            Stmt::VarDecl(name, expr) => {
                self.vars.insert(name.clone());
                if let Some(expr) = expr {
                    let expr = self.add_expr(expr);
                    self.current_block_mut().defs.insert(name.clone());
                    self.current_block_mut().var_map.insert(name.clone(), expr);
                }
            }
            Stmt::While(cond, body) => {
                let branch = self.add_expr(cond);
                let init_bb = self.blocks.len() - 1;
                let body_bb = self.add_block(BasicBlock::new());
                self.blocks[init_bb].cond = Some(branch);
                self.blocks[init_bb].true_branch = Some(body_bb);

                self.add_stmts(&body);
                let update_branch = self.add_expr(cond);
                self.current_block_mut().cond = Some(update_branch);
                self.current_block_mut().true_branch = Some(body_bb);

                let next_bb = BlockIdx(self.blocks.len());
                self.blocks[init_bb].false_branch = Some(next_bb);
                self.current_block_mut().false_branch = Some(next_bb);
            }
            Stmt::For(attrs,name, expr, expr1, body) => {
                assert!(attrs.len() <= 1);
                if attrs.len() == 1 {
                    let attr = &attrs[0].0;

                    let (start, ty)  = if let Expr::Lit(lit) = expr.as_ref() {
                        (u64::try_from(lit.value()).expect("Range start too large"), lit.ty.clone())
                    } else {
                        panic!("for loop expr not literal: {}", expr);
                    };

                    let end  = if let Expr::Lit(lit) = expr1.as_ref() {
                        u64::try_from(lit.value()).expect("Range end too large")
                    } else {
                        panic!("for loop expr not literal: {}", expr);
                    };

                    const MAX_UNROLL: usize = 10;
                    let depth = usize::try_from(end-start).expect("Too large to unroll");
                    if depth > MAX_UNROLL {
                        panic!("unroll depth too deep: {}", depth)
                    }

                    let mut first = true;

                    match &attr[..] {
                        "unroll" => {
                            for i in start..end {
                                let value = Op::Lit(TypedLit {
                                    lit: RadixIntLit::from(i),
                                    ty: ty.clone()
                                });
                                let op = self.add_op(value);
                                self.current_block_mut().var_map.insert(name.clone(), op);
                                self.current_block_mut().defs.insert(name.clone());
                                self.add_stmts(body);
                            }
                        },
                        "pipeline" => {
                            for i in start..end {
                                let value = Op::Lit(TypedLit {
                                    lit: RadixIntLit::from(i),
                                    ty: ty.clone()
                                });
                                if first {
                                    first = false;
                                } else {
                                    self.add_block(BasicBlock::new());
                                }
                                let op = self.add_op(value);
                                self.current_block_mut().var_map.insert(name.clone(), op);
                                self.current_block_mut().defs.insert(name.clone());
                                self.add_stmts(body);
                            }
                        },
                        _ => {panic!("unknown for loop attr: {}", attr)}
                    }
                } else {
                    // iterative
                    let init_value = self.add_expr(expr); // init

                    // assign name to init_value
                    self.current_block_mut().var_map.insert(name.clone(), init_value);
                    self.current_block_mut().defs.insert(name.clone());

                    let cond = {
                        let ident = Expr::Ident(name.clone());
                        let binary = Expr::Binary(
                            ast::BinaryOp::Lt, Box::new(ident), expr1.clone()
                        );
                        binary
                    };

                    let update = {
                        let ident = Expr::Ident(name.clone());
                        let one = Expr::Lit(TypedLit {
                            lit: RadixIntLit::from(1u8),
                            ty: Type::UInt(1)
                        });
                        Expr::Binary(
                            ast::BinaryOp::Add, Box::new(ident), Box::new(one)
                        )
                    };


                    let branch = self.add_expr(&cond);
                    let init_bb = self.blocks.len() - 1;
                    let body_bb = self.add_block(BasicBlock::new());
                    self.blocks[init_bb].cond = Some(branch);
                    self.blocks[init_bb].true_branch = Some(body_bb);

                    self.add_stmts(&body);
                    self.add_expr(&update);
                    let update_branch = self.add_expr(&cond);
                    self.current_block_mut().cond = Some(update_branch);
                    self.current_block_mut().true_branch = Some(body_bb);
                    let next_bb = BlockIdx(self.blocks.len());
                    self.blocks[init_bb].false_branch = Some(next_bb);
                    self.current_block_mut().false_branch = Some(next_bb);
                    self.add_block(BasicBlock::new());
                }
            }
            Stmt::CStyleFor(init, cond, update, body) => {
                if let Some(init) = init {
                    self.add_stmt(init);
                }
                let branch = if let Some(cond) = cond {
                    self.add_expr(cond)
                } else {
                    let true_lit = TypedLit {
                        lit: RadixIntLit::from(1u8),
                        ty: Type::UInt(1)
                    };
                    self.add_op(Op::Lit(true_lit))
                };
                let init_bb = self.blocks.len() - 1;
                let body_bb = self.add_block(BasicBlock::new());
                self.blocks[init_bb].cond = Some(branch);
                self.blocks[init_bb].true_branch = Some(body_bb);

                self.add_stmts(&body);
                let update_branch = if let Some(update) = update {
                    self.add_stmt(update);
                    if let Some(cond) = cond {
                        self.add_expr(cond)
                    } else {
                        let true_lit = TypedLit {
                            lit: RadixIntLit::from(1u8),
                            ty: Type::UInt(1)
                        };
                        self.add_op(Op::Lit(true_lit))
                    }
                } else {
                    branch
                };
                self.current_block_mut().cond = Some(update_branch);
                self.current_block_mut().true_branch = Some(body_bb);

                let next_bb = BlockIdx(self.blocks.len());
                self.blocks[init_bb].false_branch = Some(next_bb);
                self.current_block_mut().false_branch = Some(next_bb);
                self.add_block(BasicBlock::new());
            }
            Stmt::State(name) => {
                self.vars.insert(name.clone());
            }
        }
    }
}

impl Display for CDFG {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for (i, block) in self.blocks.iter().enumerate() {
            writeln!(f, "Block {}:", i)?;
            writeln!(f, "  Inputs: {:?}", block.inputs)?;
            writeln!(f, "  Defs: {:?}", block.defs)?;
            writeln!(f, "  Uses: {:?}", block.uses)?;
            writeln!(f, "  Ops:")?;
            for (j, op) in block.ops.iter().enumerate() {
                writeln!(f, "    {}: {:?}", j, op)?;
            }
            if let Some(cond) = block.cond {
                writeln!(f, "  Cond: {}", cond.0)?;
            }
            if let Some(true_branch) = block.true_branch {
                writeln!(f, "  True Branch: {}", true_branch.0)?;
            }
            if let Some(false_branch) = block.false_branch {
                writeln!(f, "  False Branch: {}", false_branch.0)?;
            }
        }
        Ok(())
    }
}

impl CDFG {
    pub fn regularize(&mut self) {
        // first check ExtRef ops in each block
        // if it is not in inputs, make it CDFG local var
        for block in &mut self.blocks {
            for op in block.ops.iter_mut() {
                if let Op::ExtRef(name) = op {
                    if !block.inputs.contains(name) {
                        self.vars.insert(name.clone());
                        *op = Op::StateRead(name.clone());
                    }
                }
            }
        }

        self.vars.insert("rd".to_string());

        for i in 0..self.blocks.len() {
            for var in self.vars.iter() {
                if self.blocks[i].defs.contains(var) {
                    // commit changes
                    let op = self.blocks[i].var_map.get(var).unwrap();
                    let commit = Op::StateWrite(var.clone(), *op);
                    self.blocks[i].add_op(commit);
                }
            }
        }
    }
}