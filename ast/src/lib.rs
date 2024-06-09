//! Abstract syntax tree for the toy language
//!
//! A [Program] in *toy* consists of a list of initializations ([Statement]s), a list of [Thread]s, and an assertion ([`LogicExpr`]).
//!
//! ```text
//! // Initialization
//! let x: u32 = 0;
//! let y: u32 = 0;
//!
//! // Threads
//! thread t1 {
//!   x = 1;
//!   Fence(WR);
//!   let a: u32 = x;
//! }
//! thread t2 {
//!   y = 1;
//!   Fence(WR);
//!   let b: u32 = x;
//! }
//!
//! // Assertion
//! final {
//!   assert( !( t1.a == 0 && t2.b == 0 ) );
//! }
//! ```

use std::fmt::Display;

/// The type of a variable name in *toy*
pub type Name = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Num(u32),
    Var(Name),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CondExpr {
    Neg(Box<CondExpr>),
    And(Box<CondExpr>, Box<CondExpr>),
    Eq(Expr, Expr),
    Leq(Expr, Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicInt {
    // Constant, for example `1`
    Num(u32),
    // Thread local variable, for example `t1.x`
    LogicVar(String, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicExpr {
    Neg(Box<LogicExpr>),
    And(Box<LogicExpr>, Box<LogicExpr>),
    Eq(LogicInt, LogicInt),
    Leq(LogicInt, LogicInt),
}

/// Four types of fences as defined in figure 7 of Alglave et al., 2017.
/// When a fence does not exist for a given architecture, it implies that the fence is not needed.
/// For example, an `mfence` in x86 restores Write-Read ordering. Additionally, the Write-Write relation is always respected in x86.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum FenceType {
    /// Write-Read fence exists in x86 as `mfence` and Power as `sync`
    WR,
    /// Write-Write fence does not exist in x86 and exists in Power as `sync` or `lwsync`
    WW,
    /// Read-Write fence does not exist and Power as `sync`, `lwsync`, or `dp`
    RW,
    /// Read-Read fence does not exist in x86 and exists in Power as `sync`, `lwsync`, `dp` or `isync`
    RR,
}

/// A statement in `init` in *toy*
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Init {
    /// An assignment, for example `let x: u32 = 3;`
    Assign(Name, Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// An assignment, for example `let x: u32 = 3;` or `y = x;`
    Assign(Name, Expr),
    /// A modify, for example `y = x`
    Modify(Name, Expr),
    /// A memory fence, for example `Fence(WR);`
    Fence(FenceType),
    /// An if statement, for example `if (x == 0) { ... }`
    If(CondExpr, Vec<Statement>, Vec<Statement>),
    /// A while statement, for example `while (x == 0) { ... }`
    While(CondExpr, Vec<Statement>),
}

impl From<Init> for Statement {
    fn from(value: Init) -> Self {
        match value {
            Init::Assign(name, expr) => Statement::Assign(name, expr),
        }
    }
}

/// A thread in *toy* consists of a name and a list of statements
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    /// The name of the thread
    pub name: String,
    /// Statements in the thread in the order they are written in
    pub instructions: Vec<Statement>,
}

/// A program in *toy* consists of a list of initializations, a list of threads, and an assertion
#[derive(Debug, Clone)]
pub struct Program {
    pub init: Vec<Init>,
    pub threads: Vec<Thread>,
    pub assert: Vec<LogicExpr>,
    /// Global variables that are shared between threads, all other variables are thread local
    pub global_vars: Vec<Name>,
}

/// Trait for formatting *toy* programs
trait Formatter {
    fn format(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result;
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Num(n) => write!(f, "{n}"),
            Expr::Var(x) => write!(f, "{x}"),
        }
    }
}

impl Display for CondExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CondExpr::Neg(e) => write!(f, "!({e})"),
            CondExpr::And(e1, e2) => write!(f, "({e1}) && ({e2})"),
            CondExpr::Eq(e1, e2) => write!(f, "{e1} == {e2}"),
            CondExpr::Leq(e1, e2) => write!(f, "{e1} <= {e2}"),
        }
    }
}

impl Display for LogicInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogicInt::Num(n) => write!(f, "{n}"),
            LogicInt::LogicVar(x, y) => write!(f, "{x}.{y}"),
        }
    }
}

impl Display for LogicExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogicExpr::Neg(e) => write!(f, "!({e})"),
            LogicExpr::And(e1, e2) => write!(f, "({e1}) && ({e2})"),
            LogicExpr::Eq(e1, e2) => write!(f, "{e1} == {e2}"),
            LogicExpr::Leq(e1, e2) => write!(f, "{e1} <= {e2}"),
        }
    }
}

impl Display for FenceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FenceType::WR => write!(f, "WR"),
            FenceType::WW => write!(f, "WW"),
            FenceType::RW => write!(f, "RW"),
            FenceType::RR => write!(f, "RR"),
        }
    }
}

impl Formatter for Init {
    fn format(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        match self {
            Init::Assign(name, expr) => write!(f, "{:indent$}let {name}: u32 = {expr};", "", indent = indent)
        }
    }
}

impl Display for Init {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f, 0)
    }
}

impl Formatter for Statement {
    fn format(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        match self {
            Statement::Assign(name, expr) => write!(f, "{:indent$}let {name}: u32 = {expr};", "", indent=indent),
            Statement::Modify(name, expr) => write!(f, "{:indent$}{name} = {expr};", "", indent=indent),
            Statement::Fence(fence) => write!(f, "{:indent$}Fence({fence});", "", indent=indent),
            Statement::If(cond, thn, els) => {
                writeln!(f, "{:indent$}if ({cond}) {{", "", indent=indent)?;
                for stmt in thn {
                    stmt.format(f, indent + 4)?;
                    writeln!(f)?;
                }
                writeln!(f, "{:indent$}}} else {{", "", indent=indent)?;
                for stmt in els {
                    stmt.format(f, indent + 4)?;
                    writeln!(f)?;
                }
                write!(f, "{:indent$}}}", "", indent=indent)
            }
            Statement::While(cond, body) => {
                if body.is_empty() {
                    write!(f, "{:indent$}while ({cond}) {{}}", "", indent=indent)
                } else {
                    writeln!(f, "{:indent$}while ({cond}) {{", "", indent = indent)?;
                    for stmt in body {
                        stmt.format(f, indent + 4)?;
                        writeln!(f)?;
                    }
                    write!(f, "{:indent$}}}", "", indent = indent)
                }
            }
        }
    }
}

impl Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f, 0)
    }
}

impl Formatter for Thread {
    fn format(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        writeln!(f, "{:indent$}thread {name} {{", "", name=self.name, indent=indent)?;
        for stmt in &self.instructions {
            stmt.format(f, indent + 4)?;
            writeln!(f)?;
        }
        write!(f, "{:indent$}}}", "", indent = indent)
    }
}

impl Display for Thread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(f, 0)
    }
}

impl Display for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for init in &self.init {
            init.format(f, 0)?;
            writeln!(f)?;
        }
        writeln!(f)?;

        for thread in &self.threads {
            thread.format(f, 0)?;
            write!(f, "\n\n")?;
        }

        writeln!(f, "final {{")?;
        for expr in &self.assert {
            writeln!(f, "    assert( {expr} );")?;
        }
        write!(f, "}}")
    }
}
