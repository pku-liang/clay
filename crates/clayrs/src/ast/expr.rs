use super::*;

/// `Expr` is the expression in the Clay IR.
/// It can be a literal, an identifier, a tuple, or other operations.
#[derive(Debug, Clone)]
pub enum Expr {
    /// - `Lit(typed_lit)`
    /// 
    /// a literal number value with type information
    /// e.g. `7'b1`, `8'hff`
    Lit(TypedLit),

    /// - `Ident(ident)`
    /// 
    /// a reference to a variable or a primitive function
    /// e.g. `x`, `PcUpdate` in `PcUpdate(x)`
    Ident(Ident),

    /// - `Tuple(vec)`
    /// 
    /// a tuple of expressions
    /// e.g. `(x, y)`, `(1, 2, 3)`
    Tuple(Vec<Box<Expr>>),

    /// - `Binary(binary_op, expr, expr1)`
    /// 
    /// a binary operation between two expressions
    /// e.g. `x + y`, `x * y`
    Binary(BinaryOp, Box<Expr>, Box<Expr>),

    /// - `Unary(unary_op, expr)`
    /// 
    /// a unary operation on an expression
    /// e.g. `-x`, `!x`
    Unary(UnaryOp, Box<Expr>),

    /// - `Call(ident, vec)`
    /// 
    /// a function call with a name and a vector of arguments
    /// e.g. `PcUpdate(cond, new_pc)`
    Call(Ident, Vec<Box<Expr>>),

    /// - `Repeat(times, expr)`
    /// 
    /// a repeated expression
    /// e.g. `{3{x}}`, `{5{1'b0}}`
    Repeat(u32, Box<Expr>),

    /// - `Index(expr, expr1)`
    /// 
    /// an index operation on an expression
    /// e.g. `x[0]`, `x[i]`
    Index(Box<Expr>, Box<Expr>),

    /// - `Slice(expr, expr1, expr2)`
    /// 
    /// a slice operation on an expression
    /// e.g. `x[3:0]`, `x[i:j]`
    Slice(Box<Expr>, Box<Expr>, Box<Expr>),

    /// - `Concat(vec)`
    /// 
    /// a concatenation of expressions
    /// e.g. `{x, y, z}`, `{1'b0, 1'b1, 1'b0}`
    Concat(Vec<Box<Expr>>),

    /// - `Match(expr, vec)`
    /// 
    /// a match operation on an expression
    /// e.g. `match x { 0 => y, 1 => z }`
    Match(Box<Expr>, Vec<(Box<Expr>, Box<Expr>)>),
    // Field(Box<Expr>, Ident),
    // Block(Vec<Stmt>),

    /// - `If(expr, expr1, expr2)`
    /// 
    /// an if-else statement
    /// e.g. `if x { y } else { z }`
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    // While(Box<Expr>, Box<Expr>),
    // For(Ident, Box<Expr>, Box<Expr>, Box<Expr>),
    // Return(Box<Expr>),
    // Break,
    // Continue,
    // Assign(Box<Expr>, Box<Expr>),

    /// - `Memory(expr)`
    /// 
    /// a memory operation on an expression
    /// e.g. `mem[x]`, `mem[32'h80000000]`
    Memory(Box<Expr>),
    // memread(addr)

    /// - `MemRead(expr)`
    /// 
    /// a memory read operation
    /// e.g. `memread(addr)`
    MemRead(Box<Expr>),

    /// - `MemWrite(addr, data, mask)`
    /// 
    /// a memory write operation,
    /// currently `addr` and `data` are assumed to be 32-bit and `mask` is optional byte mask.
    /// default mask is word, i.e. `0xF` (4 bytes)
    MemWrite(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
}

impl Expr {
    pub fn try_as_lit(self) -> Option<TypedLit> {
        match self {
            Expr::Lit(typed_lit) => Some(typed_lit),
            _ => None
        }
    }
}

impl Expr {
    pub fn flatten(&self) -> Vec<Box<Expr>> {
        match self {
            Expr::Tuple(vec) => {
                let mut res = vec![];
                for expr in vec {
                    res.extend(expr.flatten());
                }
                res
            }
            _ => vec![Box::new(self.clone())]
        }
    }
    pub fn is_lval(&self) -> bool {
        match self {
            Expr::Ident(_) | Expr::Index(_, _) | Expr::Slice(_, _, _) => true,
            _ => false
        }
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Lit(typed_lit) => {
                let width = typed_lit.ty.int_width();
                match width {
                    0 => write!(f, "{}", typed_lit.value()),
                    8 | 16 => write!(f, "{}'h{:x}", width, typed_lit.value()),
                    _ => write!(f, "{}'{}", width, typed_lit.lit.to_string())
                }
            }
            Expr::Ident(s) => write!(f, "{}", s),
            Expr::Tuple(vec) => {
                write!(f, "(")?;
                for (i, expr) in vec.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", expr)?;
                }
                write!(f, ")")
            }
            Expr::Binary(binary_op, expr, expr1) => {
                write!(f, "{} {} {}", expr, binary_op, expr1)
            }
            Expr::Unary(unary_op, expr) => {
                use UnaryOp::*;
                match unary_op {
                    SignedCast | UnsignedCast => {
                        write!(f, "{}({})", unary_op, expr)}
                    _ => write!(f, "{}{}", unary_op, expr)
                }
            }
            Expr::Call(ident, vec) => {
                write!(f, "{}(", ident)?;
                for (i, expr) in vec.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", expr)?;
                }
                write!(f, ")")
            },
            Expr::Repeat(times, expr) => {
                write!(f, "{{{}{{{}}}}}", times, expr)
            }
            Expr::Index(expr, expr1) => {
                write!(f, "{}[{}]", expr, expr1)
            }
            Expr::Slice(expr, expr1, expr2) => {
                write!(f, "{}[{}:{}]", expr, expr1, expr2)
            }
            Expr::Concat(vec) => {
                write!(f, "{{")?;
                for (i, expr) in vec.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", expr)?;
                }
                write!(f, "}}")
            }
            Expr::Match(expr, vec) => {
                write!(f, "match {} {{", expr)?;
                for (expr1, expr2) in vec {
                    write!(f, "{} => {}, ", expr1, expr2)?;
                }
                write!(f, "}}")
            }
            Expr::If(expr, expr1, expr2) => {
                write!(f, "if {} {{ {} }} else {{ {} }}", expr, expr1, expr2)
            }
            Expr::Memory(expr) => {
                write!(f, "mem[{}]", expr)
            }
            Expr::MemRead(expr) => {
                write!(f, "memread({})", expr)
            },
            Expr::MemWrite(expr, expr1, expr2) => {
                match expr2 {
                    Some(expr2) => write!(f, "memwrite({}, {}, {})", expr, expr1, expr2),
                    None => write!(f, "memwrite({}, {})", expr, expr1)
                }
            }
        }
    }
}

impl Expr {
    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

/// `BinaryOp` is the binary operation in the Clay IR.
/// It can be an arithmetic operation, a comparison operation, or a logical operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `+` addition
    Add,

    /// `-` subtraction
    Sub,

    /// `*` multiplication
    Mul,

    /// `/` division
    Div,

    /// `%` remainder
    Rem,

    /// `==` equality
    Eq,

    /// `!=` inequality
    Ne,

    /// `<` less than
    Lt,

    /// `<=` less than or equal to
    Le,

    /// `>` greater than
    Gt,

    /// `>=` greater than or equal to
    Ge,

    /// `&&` logical and
    And,

    /// `||` logical or
    Or,

    /// `&` bitwise and
    BitAnd,

    /// `|` bitwise or
    BitOr,

    /// `^` bitwise xor
    BitXor,

    /// `<<` left shift
    LShift,

    /// `>>` right shift
    RShift,
}


impl Display for BinaryOp {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use BinaryOp::*;
        write!(f, "{}", match self {
            Add => "+",
            Sub => "-",
            Mul => "*",
            Div => "/",
            Rem => "%",
            Eq => "==",
            Ne => "!=",
            Lt => "<",
            Le => "<=",
            Gt => ">",
            Ge => ">=",
            And => "&&",
            Or => "||",
            BitAnd => "&",
            BitOr => "|",
            BitXor => "^",
            LShift => "<<",
            RShift => ">>",
        })
    }
}



/// `UnaryOp` is the unary operation in the Clay IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-` negation
    Neg,

    /// `!` logical not
    Not,

    /// `~` bitwise not
    BitNot,

    /// `$signed` signed cast
    /// e.g. `$signed(x)`
    SignedCast,

    /// `$unsigned` unsigned cast
    /// e.g. `$unsigned(x)`
    UnsignedCast
}

impl Display for UnaryOp {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use UnaryOp::*;
        write!(f, "{}", match self {
            Neg => "-",
            Not => "!",
            BitNot => "~",
            SignedCast => "$signed",
            UnsignedCast => "$unsigned",
        })
    }
}

use cmt2::cmtir as cmt;

pub(crate) enum CmtOp {
    Cmp(cmt::Cmp),
    Prim(cmt::Prim),
}

pub(crate) trait AsCmtOp {
    fn as_cmt_op(&self) -> CmtOp;
}

impl<T: Clone + Copy> AsCmtOp for &T
where CmtOp: From<T>
{
    fn as_cmt_op(&self) -> CmtOp {
        CmtOp::from((*self).clone())
    }
}

impl From<BinaryOp> for CmtOp {
    fn from(value: BinaryOp) -> Self {
        use CmtOp::*;
        use cmt::Cmp::*;
        use cmt::Prim::*;
        match value {
            BinaryOp::Add => Prim(Add),
            BinaryOp::Sub => Prim(Sub),
            BinaryOp::Mul => Prim(Mul),
            BinaryOp::Div => Prim(Div),
            BinaryOp::Rem => Prim(Rem),
            BinaryOp::Eq => Cmp(Eq),
            BinaryOp::Ne => Cmp(Neq),
            BinaryOp::Lt => Cmp(Lt),
            BinaryOp::Le => Cmp(Leq),
            BinaryOp::Gt => Cmp(Gt),
            BinaryOp::Ge => Cmp(Geq),
            BinaryOp::And => Prim(And),
            BinaryOp::Or => Prim(Or),
            BinaryOp::BitAnd => Prim(And),
            BinaryOp::BitOr => Prim(Or),
            BinaryOp::BitXor => Prim(Xor),
            BinaryOp::LShift => Prim(DShl),
            BinaryOp::RShift => Prim(DShr),
        }
    }
}

impl From<UnaryOp> for CmtOp {
    fn from(value: UnaryOp) -> Self {
        use CmtOp::*;
        use cmt::Prim::*;
        match value {
            UnaryOp::Neg => Prim(Neg),
            UnaryOp::Not => Prim(Not),
            UnaryOp::BitNot => Prim(Not),
            UnaryOp::SignedCast => Prim(AsSInt),
            UnaryOp::UnsignedCast => Prim(AsUInt),
        }
    }
}