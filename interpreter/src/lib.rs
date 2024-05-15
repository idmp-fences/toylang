use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use ast::*;

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
    check(program);

    // Run the program
    let mut state = State::new(MemoryModel::Sc);

    init(&program.init, &mut state);
    run_threads(&program.threads, &mut state);
    assert(&program.assert, &state);
}

fn check(program: &Program) {
    todo!()
}

fn init(statements: &Vec<Init>, state: &mut State) {
    for statement in statements {
        match statement {
            Init::Assign(x, value) => {
                let value = match value {
                    Expr::Num(i) => *i,
                    Expr::Var(x) => state.read(x),
                };

                state.write_init(x, value);
            }
        }
    }
}

fn run_threads(threads: &Vec<Thread>, state: &mut State) {
    todo!()
}

fn assert(assert: &Vec<LogicExpr>, state: &State) {
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
        LogicExpr::Neg(e) => !assert_expr(e.deref(), state),
        LogicExpr::And(e1, e2) => {
            assert_expr(e1.deref(), state) && assert_expr(e2.deref(), state)
        }
        LogicExpr::Eq(e1, e2) => {
            let v1 = state.read_local(&e1.thread, &e1.variable);
            let v2 = state.read_local(&e2.thread, &e1.variable);

            v1 == v2
        }
    }
}
