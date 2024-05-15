//! Abstract syntax tree for the toy language
//!
//! A [Program] in *toy* consists of a list of initializations ([Statement]s), a list of [Thread]s, and an assertion ([`LogicExpr`]).
//!
//! ```
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

/// Four types of fences as defined in figure 7 of Alglave et al., 2017.
/// When a fence does not exist for a given architecture, it implies that the fence is not needed.
/// For example, an `mfence` in x86 restores Write-Read ordering. Additionally, the Write-Write relation is always respected in x86.
#[derive(PartialEq, Eq, Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// An assignment, for example `let x: u32 = 3;` or `y = x;`
    Assign(Name, VarInt),
    /// A memory fence, for example `Fence(WR);`
    Fence(FenceType),
}

/// The type of a variable name in *toy*
pub type Name = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarInt {
    Num(u32),
    Var(Name),
}

/// A thread in *toy* consists of a name and a list of statements
#[derive(Debug, Clone)]
pub struct Thread {
    /// The name of the thread
    pub name: String,
    /// Statements in the thread in the order they are written in
    pub instructions: Vec<Statement>,
}

/// A thread local variable in *toy*
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicVar {
    // The name of the thread
    pub thread: String,
    // The variable in the thread
    pub variable: String,
}

#[derive(Debug, Clone)]
pub enum LogicExpr {
    Neg(Box<LogicExpr>),
    And(Box<LogicExpr>, Box<LogicExpr>),
    Eq(LogicVar, LogicVar),
}

/// A program in *toy* consists of a list of initializations, a list of threads, and an assertion
#[derive(Debug, Clone)]
pub struct Program {
    pub init: Vec<Statement>,
    pub threads: Vec<Thread>,
    pub assert: LogicExpr,
}
