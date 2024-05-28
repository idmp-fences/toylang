use std::collections::{HashMap, HashSet, LinkedList};
use rand::Rng;
use ast::*;

use crate::check::check;

mod check;

#[derive(Debug, Clone)]
pub enum MemoryModel {
    Sc,
    Tso,
}

struct State {
    memory_model: MemoryModel,
    global_variables: HashSet<String>,
    memory: HashMap<String, u32>,
    write_buffers: HashMap<String, LinkedList<(String, u32)>>,
}

impl State {
    pub fn new(memory_model: MemoryModel) -> Self {
        State {
            memory_model,
            global_variables: HashSet::new(),
            memory: HashMap::new(),
            write_buffers: HashMap::new(),
        }
    }

    pub fn is_global(&self, x: &str) -> bool {
        self.global_variables.contains(x)
    }

    pub fn write_init(&mut self, x: &str, value: u32) {
        self.global_variables.insert(x.to_string());
        self.memory.insert(x.to_string(), value);
    }

    pub fn write(&mut self, x: &str, value: u32, thread: &str) {

        match self.memory_model {
            MemoryModel::Sc => {
                self.memory.insert(x.to_string(), value);
            }
            MemoryModel::Tso => {
                if let Some(buffer) = self.write_buffers.get_mut(thread) {
                    buffer.push_back((x.to_string(), value));
                }
            }
        }
    }

    pub fn write_local(&mut self, thread: &str, x: &str, value: u32) {
        self.memory.insert(format!("{thread}.{x}"), value);
    }

    pub fn read(&self, x: &str) -> u32 {
        match self.memory_model {
            MemoryModel::Sc => {
                self.memory.get(x).copied().unwrap_or(0)
            }
            MemoryModel::Tso => {
                self.memory.get(x).copied().unwrap_or(0)
            }
        }
       
    }

    pub fn read_local(&self, thread: &str, x: &str) -> u32 {
        match self.memory_model {
            MemoryModel::Sc => {
                self.read(format!("{thread}.{x}").as_str())
            }
            MemoryModel::Tso => {
                self.read(format!("{thread}.{x}").as_str())
            }
        }
    }

    /// Flushes a random thread-local variable to global variables for a specified thread.
    pub fn flush_single_write_buffer(&mut self, thread_name: &str) -> bool {
        if let Some(buffer) = self.write_buffers.get_mut(thread_name) {
            let result = buffer.pop_front();
            return match result {
                Some((key, value)) => {
                    self.memory.insert(key, value);
                    true
                },
                None => {
                    false
                }
            }
        }
        false 
    }

    
    /// Continues to flush random write buffers for a specific thread until all are flushed.
    pub fn flush_write_buffer(&mut self, thread_name: &str) {
        while self.flush_single_write_buffer(thread_name) {}
    }
}

pub fn execute(program: &Program, memory_model: MemoryModel) -> bool {
    // Check if program is valid
    check(program).unwrap_or_else(|err| panic!("{err:?}"));

    // Run the program
    let mut state = State::new(memory_model);

    init(&program.init, &mut state);
    run_threads(&program.threads, &mut state);
    assert(&program.assert, &state)
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
    let mut rng = rand::thread_rng(); // Create a random number generator
    let mut active_threads = (0..threads.len()).collect::<Vec<_>>(); // Track active threads by their indices
    let mut ip = vec![vec![0]; threads.len()]; // Instruction pointers for each thread
    
    for thread in threads {
        state.write_buffers.entry(thread.name.clone()).or_default();
    }

    while !active_threads.is_empty() {
        // Randomly select an active thread
        let idx = rng.gen_range(0..active_threads.len());
        let thread_idx = active_threads[idx];

        // Run the next instruction if there is one
        if ip[thread_idx][0] < threads[thread_idx].instructions.len() {
            let instruction = &threads[thread_idx].instructions[ip[thread_idx][0]];
            let cont = simulate_instruction(instruction, &threads[thread_idx].name, state, &mut ip[thread_idx], 1);
            if cont {
                ip[thread_idx][0] += 1; // Move the instruction pointer forward
            }

            // With a 25% chance, flush one random write buffer item
            match state.memory_model {
                MemoryModel::Sc => {
                    //
                }
                MemoryModel::Tso => {
                    let mut rng = rand::thread_rng();
                    if rng.gen::<f64>() < 0.25 {
                        state.flush_single_write_buffer(&threads[thread_idx].name);  // Ensure flush_random_write_buffer accepts thread_name
                    }
                }
            }

            // Check if this thread has completed all its instructions
            if ip[thread_idx][0] >= threads[thread_idx].instructions.len() {
                active_threads.swap_remove(idx); // Remove the thread from the active list
            }
        }
    }
}

fn simulate_instruction(instruction: &Statement, thread_name: &str, state: &mut State, ip: &mut Vec<usize>, d: usize) -> bool {
    match instruction {
        Statement::Modify(var, expr) => {
            let value = evaluate_expression(expr, state, thread_name);
            if state.is_global(var) {
                state.write(var, value, thread_name);
            } else {
                state.write_local(thread_name, var, value);
            }
            
        },
        Statement::Assign(var, expr) => {
            let value = evaluate_expression(expr, state, thread_name);
            // Assign to a local/thread-specific variable
            state.write_local(thread_name, var, value);
        },
        Statement::Fence(fence_type) => {
            // Apply the specified fence
            if fence_type == &FenceType::WR {
                state.flush_write_buffer(thread_name);
            }
        },
        Statement::If(cond, thn, els) => {
            // Check if pointer is currently in a branch
            if ip.len() > d {
                let (instr, end) = match ip[d] < thn.len() {
                    true => (&thn[ip[d]], thn.len()),
                    false => (&els[ip[d] - thn.len()], thn.len() + els.len())
                };

                return if simulate_instruction(instr, thread_name, state, ip, d + 1) {
                    ip[d] += 1;
                    // Check if pointer has reached the end of the branch
                    if ip[d] == end {
                        ip.pop();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
            }

            if evaluate_cond_expression(cond, state, thread_name) {
                ip.push(0)
            } else {
                ip.push(thn.len())
            }
            return false
        }
        Statement::While(cond, body) => {
            // Check if pointer is currently in a branch
            if ip.len() > d {
                // Check if pointer has reached the end of the body and the condition needs to be evaluated
                if ip[d] == body.len() {
                    return if evaluate_cond_expression(cond, state, thread_name) {
                        ip[d] = 0;
                        false
                    } else {
                        ip.pop();
                        true
                    }
                }

                if simulate_instruction(&body[ip[d]], thread_name, state, ip, d + 1) {
                    ip[d] += 1;
                }
                return false;
            }

            if evaluate_cond_expression(cond, state, thread_name) {
                ip.push(0);
                return false;
            }
        }
    }

    true
}

fn evaluate_expression(expr: &Expr, state: &mut State, thread_name: &str) -> u32 {
    match expr {
        Expr::Num(val) => *val,
        Expr::Var(var) => {
            if var.contains('.') {
                let parts: Vec<&str> = var.split('.').collect();
                state.read_local(parts[0], parts[1])
            } 
            else if state.memory.contains_key(&[thread_name,".",var].join("")) {
                state.read_local(thread_name, var)
            } else {
                state.read(var)
            }
        },
    }
}

fn evaluate_cond_expression(expr: &CondExpr, state: &mut State, thread_name: &str) -> bool {
    match expr {
        CondExpr::Neg(e) => !evaluate_cond_expression(e, state, thread_name),
        CondExpr::And(e1, e2) =>
            evaluate_cond_expression(e1, state, thread_name) && evaluate_cond_expression(e2, state, thread_name),
        CondExpr::Eq(e1, e2) =>
            evaluate_expression(e1, state, thread_name) == evaluate_expression(e2, state, thread_name),
    }
}

fn assert(assert: &[LogicExpr], state: &State) -> bool {
    for (i, logic_expr) in assert.iter().enumerate() {
        let result = assert_expr(logic_expr, state);
        if !result {
            println!("Assertion {} failed", i);
            dbg!(assert);
            return false;
        }
    }

    true
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

#[cfg(test)]
mod tests {
    use ast::Expr::{Num, Var};
    // Import necessary components from the outer module
    use super::*;

    #[test]
    fn test_local_global_vars() {
        let memory_model = MemoryModel::Tso;
        let init = vec![
            Init::Assign("x".to_string(), Expr::Num(10)), 
        ];
        let threads = vec![
            Thread {
                name: "t1".to_string(),
                instructions: vec![
                    Statement::Assign("x".to_string(), Expr::Num(100)),
                ],
            }
        ];
        let assert = vec![];

        let program = Program {
            init,
            threads,
            assert,
            global_vars: vec!["x".to_string()],
        };
        let mut state = State::new(memory_model);
        crate::init(&program.init, &mut state);
        run_threads(&program.threads, &mut state);


        assert_eq!(state.read("x"),10);
        assert_eq!(state.read_local("t1","x"),100);
    }

    #[test]
    fn test_read_writes() {
        let memory_model = MemoryModel::Tso;
        let init = vec![
                    Init::Assign("x".to_string(), Expr::Num(10)),
                    Init::Assign("y".to_string(), Expr::Num(20)),
                    Init::Assign("z".to_string(), Expr::Num(30)),
        ];
        let threads = vec![
            Thread {
                name: "t1".to_string(),
                instructions: vec![
                    Statement::Modify("x".to_string(), Expr::Num(100)),
                    Statement::Modify("y".to_string(), Expr::Num(200)),
                    Statement::Modify("z".to_string(), Expr::Num(300)),
                    Statement::Fence(FenceType::WR),
                    Statement::Assign("fencedX".to_string(), Expr::Var("x".to_string())),
                    Statement::Assign("fencedY".to_string(), Expr::Var("y".to_string())),
                    Statement::Assign("fencedZ".to_string(), Expr::Var("z".to_string())),
                ],
            }
        ];
        let assert = vec![];

        let program = Program {
            init,
            threads,
            assert,
            global_vars: vec!["x".to_string(), "y".to_string(), "z".to_string()],
        };
        let mut state = State::new(memory_model);
        crate::init(&program.init, &mut state);
        run_threads(&program.threads, &mut state);
        assert_eq!(state.read_local("t1","fencedX"), 100);
        assert_eq!(state.read_local("t1","fencedY"), 200);
        assert_eq!(state.read_local("t1","fencedZ"), 300);
    }

    #[test]
    fn test_thread_end() {
        let mut assertion_failed = false;

        for _ in 0..100 {
            let memory_model = MemoryModel::Tso;
            let init = vec![
                Init::Assign("x".to_string(), Num(10)),
            ];
            let threads = vec![
                Thread {
                    name: "t1".to_string(),
                    instructions: vec![
                        Statement::Modify("x".to_string(), Num(100)),
                    ],
                }
            ];
            let assert = vec![];

            let program = Program {
                init,
                threads,
                assert,
                global_vars: vec!["x".to_string()],
            };
            let mut state = State::new(memory_model);
            run_threads(&program.threads, &mut state);

            // Check if the state does not match expected
            if state.read("x") != 100 {
                assertion_failed = true;
                break; // Since you only need one failure, you can break early
            }
        }

        assert!(assertion_failed, "The assertion was never false in 100 runs.");
    }

    #[test]
    fn test_read_writes_2() {
        let memory_model = MemoryModel::Tso;

        let mut state = State::new(memory_model);
        state.write_buffers.entry("t1".to_string()).or_default();
        if let Some(buffer) = state.write_buffers.get_mut("t1") {
            buffer.push_back(("x".to_string(), 0u32));
        }
        state.write("x", 1, "t1");
        state.write("y", 2, "t1");
        state.write("z", 3, "t1");
        assert_ne!(state.read("x"),1);
        assert_ne!(state.read("y"),2);
        assert_ne!(state.read("z"),3);
        state.flush_write_buffer("t1");
        assert_eq!(state.read("x"),1);
        assert_eq!(state.read("y"),2);
        assert_eq!(state.read("z"),3);
    }

    #[test]
    fn test_sc_writes() {
        let memory_model = MemoryModel::Sc;
        let mut state = State::new(memory_model);
        state.write("x", 1, "main");
        state.write("y", 2, "main");
        state.write("z", 3, "main");
        assert_eq!(state.read("x"),1);
        assert_eq!(state.read("y"),2);
        assert_eq!(state.read("z"),3);
        state.write_local("t1","x", 11);
        state.write_local("t2","y", 22);
        state.write_local("t2","z", 33);
        
        assert_eq!(state.read("x"), 1);
        assert_eq!(state.read("y"), 2);
        assert_eq!(state.read("z"), 3);
        assert_eq!(state.read_local("t1","x"), 11);
        assert_eq!(state.read_local("t2","y"), 22);
        assert_eq!(state.read_local("t2","z"), 33);
    }

    #[test]
    fn test_if_true() {
        let memory_model = MemoryModel::Tso;
        let init = vec![];
        let threads = vec![
            Thread {
                name: "t1".to_string(),
                instructions: vec![
                    Statement::Assign("a".to_string(), Num(0)),
                    Statement::If(CondExpr::Eq(Var("a".to_string()), Num(0)), vec![
                        Statement::Modify("a".to_string(), Num(1)),
                    ], vec![
                        Statement::Modify("a".to_string(), Num(2)),
                    ]),
                ],
            }
        ];
        let assert = vec![
            LogicExpr::Eq(LogicInt::LogicVar("t1".to_string(), "a".to_string()), LogicInt::Num(1)),
        ];
        
        let program = Program { init, threads, assert, global_vars: vec![] };
        let result = execute(&program, memory_model);
        assert!(result);
    }
    
    #[test]
    fn test_if_false() {
        let memory_model = MemoryModel::Tso;
        let init = vec![];
        let threads = vec![
            Thread {
                name: "t1".to_string(),
                instructions: vec![
                    Statement::Assign("a".to_string(), Num(0)),
                    Statement::If(CondExpr::Eq(Var("a".to_string()), Num(1)), vec![
                        Statement::Modify("a".to_string(), Num(1)),
                    ], vec![
                        Statement::Modify("a".to_string(), Num(2)),
                    ]),
                ],
            }
        ];
        let assert = vec![
            LogicExpr::Eq(LogicInt::LogicVar("t1".to_string(), "a".to_string()), LogicInt::Num(2)),
        ];
        
        let program = Program { init, threads, assert, global_vars: vec![] };
        let result = execute(&program, memory_model);
        assert!(result);
    }

    #[test]
    fn test_while() {
        let memory_model = MemoryModel::Tso;
        let init = vec![];
        let threads = vec![
            Thread {
                name: "t1".to_string(),
                instructions: vec![
                    Statement::Assign("a".to_string(), Num(2)),
                    Statement::Assign("b".to_string(), Num(1)),
                    Statement::Assign("c".to_string(), Num(0)),
                    Statement::While(CondExpr::Neg(Box::from(CondExpr::Eq(Var("a".to_string()), Num(0)))), vec![
                        Statement::Modify("a".to_string(), Var("b".to_string())),
                        Statement::Modify("b".to_string(), Var("c".to_string())),
                        Statement::Fence(FenceType::WW),
                    ]),
                ],
            }
        ];
        let assert = vec![
            LogicExpr::Eq(LogicInt::LogicVar("t1".to_string(), "a".to_string()), LogicInt::Num(0)),
        ];

        let program = Program { init, threads, assert, global_vars: vec![] };
        let result = execute(&program, memory_model);

        assert!(result);
    }
}