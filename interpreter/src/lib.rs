mod check;

use std::collections::{HashMap, HashSet};

use ast::*;
use crate::check::check;

#[derive(Debug, Clone)]
enum MemoryModel {
    Sc,
    Tso,
}

#[derive(Debug, Clone)]
struct State {
    memory_model: MemoryModel,
    global_variables: HashSet<String>,
    memory: HashMap<String, u32>,
    // TODO: write_buffers: ?,
}

impl State {
    pub fn new(memory_model: MemoryModel) -> Self {
        State {
            memory_model,
            global_variables: HashSet::new(),
            memory: HashMap::new(),
        }
    }

    pub fn is_global(&self, x: &str) -> bool {
        self.global_variables.contains(x)
    }

    pub fn write_init(&mut self, x: &str, value: u32) {
        self.global_variables.insert(x.to_string());
        self.memory.insert(x.to_string(), value);
    }

    pub fn write(&mut self, x: &str, value: u32) {
        match self.memory_model {
            MemoryModel::Sc => {
                self.memory.insert(x.to_string(), value);
            }
            MemoryModel::Tso => {
                todo!()
            }
        }
    }

    pub fn write_local(&mut self, thread: &str, x: &str, value: u32) {
        self.memory.insert(format!("{thread}.{x}"), value);
    }

    pub fn read(&self, x: &str) -> u32 {
        self.memory.get(x).copied().unwrap_or(0)
    }

    pub fn read_local(&self, thread: &str, x: &str) -> u32 {
        self.read(format!("{thread}.{x}").as_str())
    }

    pub fn flush_write_buffer(&mut self) {
        todo!()
    }
}

pub fn execute(program: &Program) {
    // Check if program is valid
    check(program).unwrap_or_else(|err| panic!("{err:?}"));

    // Run the program
    let mut state = State::new(MemoryModel::Sc);

    init(&program.init, &mut state);
    run_threads(&program.threads, &mut state);
    assert(&program.assert, &state);
}

fn init(statements: &[Init], state: &mut State) {
    for statement in statements {
        match statement {
            Init::Assign(x, expr) => {
                let value = match expr {
                    Expr::Num(i) => *i,
                    Expr::Var(x) => state.read(x),
                };

                state.write_init(x, value);
            }
        }
    }
}

fn run_threads(threads: &[Thread], state: &mut State) {
    todo!()
}

fn assert(assert: &[LogicExpr], state: &State) {
    for (i, logic_expr) in assert.iter().enumerate() {
        let result = assert_expr(logic_expr, state);
        if !result {
            println!("Assertion failed: {}", i);
            dbg!(state);
            dbg!(assert);
        }
    }
}

fn assert_expr(expr: &LogicExpr, state: &State) -> bool {
    match expr {
        LogicExpr::Neg(e) => !assert_expr(e, state),
        LogicExpr::And(e1, e2) => {
            assert_expr(e1, state) && assert_expr(e2, state)
        }
        LogicExpr::Eq(e1, e2) => {
            let v1 = assert_logic_int(e1, state);
            let v2 = assert_logic_int(e2, state);

            v1 == v2
        }
    }
}

fn assert_logic_int(expr: &LogicInt, state: &State) -> u32 {
    match expr {
        LogicInt::Num(i) => *i,
        LogicInt::LogicVar(thread, variable) => state.read_local(thread, variable),
    }
}
