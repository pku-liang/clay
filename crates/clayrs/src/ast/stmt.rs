use super::*;
use expr::Expr;

/// `Stmt` is the statement in the Clay IR.
/// It can be an assignment, an expression, a variable declaration,

#[derive(Debug, Clone)]
pub enum AssignKind {
    Let,
    Var,
    Update
}

#[derive(Debug, Clone)]
pub enum Stmt {
    // Const(Ident, Box<Expr>),

    /// - `Assign(is_let, lhs, rhs)`
    /// - `let lhs = rhs;` (is_let = true)
    /// - `lhs = rhs;` (is_let = false)
    /// 
    /// Difference between `let =` and `=` is that `let =` rebinds the identifier to a new value,
    /// while `=` updates the value of the identifier. (only for stateful variables)
    Assign(AssignKind, Box<Expr>, Box<Expr>),

    /// - `Expr(expr)`
    /// - `expr;`
    /// 
    /// Holds a statement with singl expression
    /// such as `PcUpdate(cond, new_pc);`
    Expr(Box<Expr>),

    /// - `VarDecl(ident, expr)`
    /// - `var ident = expr;`
    /// 
    /// Variable declaration for stateful ISAX
    /// corresponds to a local register in the architecture
    VarDecl(Ident, Option<Box<Expr>>),

    /// - `While(expr, body)`
    /// - `while(cond) { body }`
    /// 
    /// while loop statement
    /// `body` is a vector of statements
    While(Box<Expr>, Vec<Stmt>),

    /// - `For(ident, start, end, body)`
    /// - `for ident in (start..end) { body }`
    /// 
    /// for loop statement
    /// `body` is a vector of statements
    For(Vec<(String, Option<Box<Expr>>)>, Ident, Box<Expr>, Box<Expr>, Vec<Stmt>),

    CStyleFor(Option<Box<Stmt>>, Option<Box<Expr>>, Option<Box<Stmt>>, Vec<Stmt>),

    /// - `State(ident)`
    /// - `state ident;`
    /// 
    /// State declaration for stateful ISAX
    /// corresponds to a global CSR in the architecture
    State(Ident)
}

impl Display for Stmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Stmt::Assign(kind, expr, expr1) => {
                // if *is_let {
                    // write!(f, "let {} = {};", expr, expr1)
                // } else {
                write!(f, "{}", match kind {
                    AssignKind::Let => "let ",
                    AssignKind::Var => "var ",
                    AssignKind::Update => "",
                })?;
                write!(f, "{} = {};", expr, expr1)
                // }
            }
            Stmt::Expr(expr) => {
                write!(f, "{};", expr)
            }
            Stmt::VarDecl(ident, expr) => {
                if let Some(expr) = expr {
                    write!(f, "let {} = {};", ident, expr)
                } else {
                    write!(f, "let {};", ident)
                }
            }
            Stmt::While(expr, v) => {
                write!(f, "while ({}) {{", expr)?;
                for stmt in v {
                    write!(f, "  {}", stmt)?;
                }
                write!(f, "}}")
            }
            Stmt::For(attrs, var, start, end, v) => {
                // let start = Expr::Lit(start.clone());
                // let end = Expr::Lit(end.clone());
                write!(f, "#[{:?}] for {} in ({}:{}) {{", attrs, var, start, end)?;
                for stmt in v {
                    write!(f, "  {}", stmt)?;
                }
                write!(f, "}}")
            }
            Stmt::State(ident) => {
                write!(f, "state {}", ident)
            }
            Stmt::CStyleFor(init, cond, update, body) => {
                write!(f, "for ({};{};{}) {{", 
                    init.as_ref().map(|e| e.to_string()).unwrap_or_default(),
                    cond.as_ref().map(|e| e.to_string()).unwrap_or_default(),
                    update.as_ref().map(|e| e.to_string()).unwrap_or_default())?;
                for stmt in body {
                    write!(f, "  {}", stmt)?;
                }
                write!(f, "}}")
            }
        }
    }
}