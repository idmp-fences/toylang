mod check;

use std::collections::{HashMap, HashSet};
use rand::{Rng, thread_rng};

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
    write_buffers: HashMap<String, HashMap<String, u32>>,
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
                    buffer.insert(x.to_string(), value);
                } else {
                    self.memory.insert(x.to_string(), value);
                }
            }
        }
    }

    pub fn write_local(&mut self, thread: &str, x: &str, value: u32) {
        match self.memory_model {
            MemoryModel::Sc => {
                self.memory.insert(format!("{thread}.{x}"), value);
            }
            MemoryModel::Tso => {
                if let Some(buffer) = self.write_buffers.get_mut(thread) {
                    buffer.insert(format!("{thread}.{x}"), value);
                } else {
                    //
                }
            }
        }
    }

    pub fn read(&mut self, x: &str, thread: &str) -> u32 {
        match self.memory_model {
            MemoryModel::Sc => {
                self.memory.get(x).copied().unwrap_or(0)
            }
            MemoryModel::Tso => {
                if let Some(buffer) = self.write_buffers.get_mut(thread) {
                    if buffer.contains_key(x) {
                        buffer.get(x).copied().unwrap_or(0)
                    } else {
                        self.memory.get(x).copied().unwrap_or(0)
                    }
                    
                } else {
                    // this happens when main thread executes
                    self.memory.get(x).copied().unwrap_or(0)
                }
            }
        }
       
    }

    pub fn read_local(&mut self, thread: &str, x: &str) -> u32 {
        match self.memory_model {
            MemoryModel::Sc => {
                self.read(format!("{thread}.{x}").as_str(), thread)
            }
            MemoryModel::Tso => {
                if let Some(buffer) = self.write_buffers.get_mut(thread) {
                    if buffer.contains_key(format!("{thread}.{x}").as_str()) {
                        buffer.get(format!("{thread}.{x}").as_str()).copied().unwrap_or(0)
                    } else {
                        self.read(format!("{thread}.{x}").as_str(), thread)
                    }
                } else {
                    self.read(format!("{thread}.{x}").as_str(), thread)
                }
            }
        }
    }

    /// Flushes a random thread-local variable to global variables for a specified thread.
    pub fn flush_random_write_buffer(&mut self, thread_name: &str) -> bool {
        if let Some(buffer) = self.write_buffers.get_mut(thread_name) {
            let keys: Vec<String> = buffer.keys().cloned().collect();
            if keys.is_empty() {
                return false; 
            }

            let mut rng = thread_rng();
            let random_index = rng.gen_range(0..keys.len());
            let random_key = &keys[random_index];

            if let Some(value) = buffer.remove(random_key) {
                self.memory.insert(random_key.clone(), value);
                return true; 
            }
        }
        false 
    }

    
    /// Continues to flush random write buffers for a specific thread until all are flushed.
    pub fn flush_write_buffer(&mut self, thread_name: &str) {
        while self.flush_random_write_buffer(thread_name) {
            // Keep flushing while there are buffer variables
        }
    }
}

pub fn execute(program: &Program, memory_model: MemoryModel) {
    // Check if program is valid
    check(program).unwrap_or_else(|err| panic!("{err:?}"));

    // Run the program
    let mut state = State::new(memory_model);

    init(&program.init, &mut state);
    run_threads(&program.threads, &mut state);
    assert(&program.assert, &mut state);
}

fn init(statements: &[Init], state: &mut State) {
    for statement in statements {
        match statement {
            Init::Assign(x, expr) => {
                let value = match expr {
                    Expr::Num(i) => *i,
                    Expr::Var(x) => state.read(x, "main"),
                };

                state.write_init(x, value);
            }
        }
    }
}

fn run_threads(threads: &[Thread], state: &mut State) {
    let mut rng = rand::thread_rng(); // Create a random number generator
    let mut active_threads = (0..threads.len()).collect::<Vec<_>>(); // Track active threads by their indices
    let mut ip = vec![0; threads.len()]; // Instruction pointers for each thread
    
    for thread in threads {
        state.write_buffers.entry(thread.name.clone()).or_insert_with(HashMap::new);
    }

    while !active_threads.is_empty() {
        // Randomly select an active thread
        let idx = rng.gen_range(0..active_threads.len());
        let thread_idx = active_threads[idx];

        // Run the next instruction if there is one
        if ip[thread_idx] < threads[thread_idx].instructions.len() {
            let instruction = &threads[thread_idx].instructions[ip[thread_idx]];
            simulate_instruction(instruction, &threads[thread_idx].name, state);
            ip[thread_idx] += 1; // Move the instruction pointer forward

            // Check if this thread has completed all its instructions
            if ip[thread_idx] >= threads[thread_idx].instructions.len() {
                state.flush_write_buffer(&threads[thread_idx].name);
                active_threads.swap_remove(idx); // Remove the thread from the active list
            }
        }
    }
}

fn simulate_instruction(instruction: &Statement, thread_name: &str, state: &mut State) {
    match instruction {
        Statement::Modify(var, expr) => {
            let value = evaluate_expression(expr, state, thread_name);
            if state.memory.contains_key(format!("{thread_name}.{var}").as_str()) {
                state.write_local(thread_name, var, value); // Modify the global variable
            } else {
                state.write(var, value, thread_name);
            }
            
        },
        Statement::Assign(var, expr) => {
            let value = evaluate_expression(expr, state, thread_name);
            state.write_local(thread_name, var, value); // Assign to a local/thread-specific variable
        },
        Statement::Fence(fence_type) => {
            apply_fence(fence_type, state, thread_name); // Apply the specified fence
            
        },
    }
     // With a 25% chance, flush one random write buffer item
    match state.memory_model {
        MemoryModel::Sc => {
            // 
        }
        MemoryModel::Tso => {
            let mut rng = rand::thread_rng();
            if rng.gen::<f64>() < 0.25 {
                state.flush_random_write_buffer(thread_name);  // Ensure flush_random_write_buffer accepts thread_name
            }
        }
    }
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
                state.read(var, thread_name)
            }
        },
        _ => unreachable!(), // Handle other expressions as needed
    }
}

fn apply_fence(fence_type: &FenceType, state: &mut State, thread_name: &str) {
    match fence_type {
        FenceType::WR => {
            // Implement the logic for write-read fence
            state.flush_write_buffer(thread_name);
        },
        _ => {
            // Handle other types of fences as required
        },
    }
}


fn assert(assert: &[LogicExpr], state: &mut State) {
    for (i, logic_expr) in assert.iter().enumerate() {
        let result = assert_expr(logic_expr, state);
        if !result {
            // dbg!(state);
            // dbg!(assert);
        }
    }
}

fn assert_expr(expr: &LogicExpr, state: &mut State) -> bool {
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

fn assert_logic_int(expr: &LogicInt, state: &mut State) -> u32 {
    match expr {
        LogicInt::Num(i) => *i,
        LogicInt::LogicVar(thread, variable) => state.read_local(thread, variable),
    }
}

fn print_thread_data(write_buffers: &HashMap<String, HashMap<String, u32>>, thread_name: &str) {
    if let Some(buffer) = write_buffers.get(thread_name) {
        println!("Data for thread '{}':", thread_name);
        for (key, value) in buffer {
            println!("{}: {}", key, value);
        }
    } else {
        println!("No data found for thread '{}'.", thread_name);
    }
}

fn print_memory(memory: &HashMap<String, u32>) {
    for (key, value) in memory {
        println!("{}: {}", key, value);
    }
}
#[cfg(test)]
mod tests {
    use super::*;  // Import necessary components from the outer module

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
        };
        let mut state = State::new(memory_model);
        crate::init(&program.init, &mut state);
        run_threads(&program.threads, &mut state);


        assert_eq!(state.read("x", "main"),10);
        assert_eq!(state.read_local("t1","x"),100);
    }

    #[test]
    fn test_read_writes() {
        let memory_model = MemoryModel::Tso;
        let init = vec![];
        let threads = vec![
            Thread {
                name: "t1".to_string(),
                instructions: vec![
                    Statement::Assign("x".to_string(), Expr::Num(10)),
                    Statement::Assign("y".to_string(), Expr::Num(20)),
                    Statement::Assign("z".to_string(), Expr::Num(30)),
                    Statement::Fence(FenceType::WR),
                    Statement::Modify("x".to_string(), Expr::Num(100)),
                    Statement::Modify("y".to_string(), Expr::Num(200)),
                    Statement::Modify("z".to_string(), Expr::Num(300)),
                    Statement::Assign("fencedX".to_string(), Expr::Var("t1.x".to_string())),
                    Statement::Assign("fencedY".to_string(), Expr::Var("t1.y".to_string())),
                    Statement::Assign("fencedZ".to_string(), Expr::Var("t1.z".to_string())),
                ],
            }
        ];
        let assert = vec![];

        let program = Program {
            init,
            threads,
            assert,
        };
        let mut state = State::new(memory_model);
        run_threads(&program.threads, &mut state);
        assert_eq!(state.read_local("t1","fencedX"), 100);
        assert_eq!(state.read_local("t1","fencedY"), 200);
        assert_eq!(state.read_local("t1","fencedZ"), 300);
    }

    #[test]
    fn test_thread_end() {
        let memory_model = MemoryModel::Tso;
        let init = vec![
            Init::Assign("x".to_string(), Expr::Num(10)), 
        ];
        let threads = vec![
            Thread {
                name: "t1".to_string(),
                instructions: vec![
                    Statement::Modify("x".to_string(), Expr::Num(100)),
                ],
            }
        ];
        let assert = vec![];

        let program = Program {
            init,
            threads,
            assert,
        };
        let mut state = State::new(memory_model);
        run_threads(&program.threads, &mut state);
        assert_eq!(state.read("x", "main"),100);
    }

    #[test]
    fn test_read_writes_2() {
        let memory_model = MemoryModel::Tso;
        let init = vec![];
        let threads = vec![];
        let assert = vec![];

        let program = Program {
            init,
            threads,
            assert,
        };
        let mut state = State::new(memory_model);
        state.write_buffers.entry("t1".to_string()).or_insert_with(HashMap::new);
        if let Some(buffer) = state.write_buffers.get_mut("t1") {
            buffer.insert(format!("x"), 0u32);
        }
        state.write("x", 1, "main");
        state.write("y", 2, "main");
        state.write("z", 3, "main");
        assert_eq!(state.read("x", "main"),1);
        assert_eq!(state.read("y", "main"),2);
        assert_eq!(state.read("z", "main"),3);
        state.write_local("t1","x", 11);
        assert_eq!(state.read("x", "main"),1);
        assert_ne!(state.read("t1.x", "main"),11);
        state.flush_write_buffer("t1");
        assert_eq!(state.read("t1.x", "main"),11);
    }

    #[test]
    fn test_sc_writes() {
        let memory_model = MemoryModel::Sc;
        let init = vec![];
        let threads = vec![];
        let assert = vec![];

        let program = Program {
            init,
            threads,
            assert,
        };
        let mut state = State::new(memory_model);
        state.write("x", 1, "main");
        state.write("y", 2, "main");
        state.write("z", 3, "main");
        assert_eq!(state.read("x", "main"),1);
        assert_eq!(state.read("y", "main"),2);
        assert_eq!(state.read("z", "main"),3);
        state.write_local("t1","x", 11);
        state.write_local("t2","y", 22);
        state.write_local("t2","z", 33);
        
        assert_eq!(state.read("x", "main"), 1);
        assert_eq!(state.read("y", "main"), 2);
        assert_eq!(state.read("z", "main"), 3);
        assert_eq!(state.read_local("t1","x"), 11);
        assert_eq!(state.read_local("t2","y"), 22);
        assert_eq!(state.read_local("t2","z"), 33);
    }

}