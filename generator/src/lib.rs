use ast::*;
use ast::CondExpr::*;
use ast::Expr::*;
use ast::Statement::*;
use rand::{Rng, RngCore, thread_rng};

/// Generates a random program.
pub fn generate_random_program(
    num_threads: usize,
    num_instructions: usize,
    num_global_variables: usize,
    max_depth: usize
) -> Program {
    generate_random_program_with_rng(num_threads, num_instructions, num_global_variables, max_depth, &mut thread_rng())
}

pub fn generate_random_program_with_rng<R: RngCore + ?Sized>(
    num_threads: usize,
    num_instructions: usize,
    num_global_variables: usize,
    max_depth: usize,
    rng: &mut R
) -> Program {
    if num_threads == 0 {
        panic!("Number of threads must be at least 1.")
    } else if num_instructions == 0 {
        panic!("Number of instructions must be at least 1.")
    } else if num_global_variables == 0 {
        panic!("Number of global variables must be at least 1.")
    }

    // Init
    let mut global_vars = vec![];
    let mut init = vec![];
    for i in 0..num_global_variables {
        global_vars.push(format!("x{}", i));
        init.push(Init::Assign(format!("x{}", i), Num(0)));
    }

    // Threads
    let mut threads = vec![];
    for i in 0..num_threads {
        let instructions = generate_random_instructions(num_instructions, num_global_variables, num_global_variables / 2, max_depth, rng);
        threads.push(Thread {
            name: format!("t{}", i),
            instructions,
        });
    }

    Program {
        global_vars,
        init,
        threads,
        assert: vec![],
    }
}

fn generate_random_instructions<R: RngCore + ?Sized>(
    num_instructions: usize,
    num_global_variables: usize,
    num_values: usize,
    max_depth: usize,
    rng: &mut R
) -> Vec<Statement> {
    let mut instructions = vec![];
    for j in 0..num_instructions {
        let choice = if max_depth == 0 { rng.gen::<f64>() * 0.8 + 0.2 } else { rng.gen::<f64>() };
        if choice < 0.1 {
            // If statement
            let lhs = generate_random_expression(num_global_variables, num_values, rng);
            let rhs = Num(generate_random_value(num_values, rng));
            instructions.push(If(
                Eq(lhs, rhs),
                generate_random_instructions(num_instructions / 2, num_global_variables, num_values, max_depth - 1, rng),
                generate_random_instructions(num_instructions / 2, num_global_variables, num_values, max_depth - 1, rng),
            ));
        } else if choice < 0.2 {
            // While statement
            let lhs = generate_random_expression(num_global_variables, num_values, rng);
            let rhs = Num(generate_random_value(num_values, rng));
            instructions.push(While(
                Eq(lhs, rhs),
                generate_random_instructions(num_instructions / 2, num_global_variables, num_values, max_depth - 1, rng),
            ));
        } else if choice < 0.6 {
            // Write global variable
            let variable = rng.gen_range(0..num_global_variables);
            let value = generate_random_value(num_values, rng);
            instructions.push(Modify(format!("x{}", variable), Num(value)));
        } else {
            // Read global variable
            let variable = rng.gen_range(0..num_global_variables);
            instructions.push(Assign(format!("y{}", j), Var(format!("x{}", variable))));
        }
    }

    instructions
}

fn generate_random_expression<R: RngCore + ?Sized>(num_global_variables: usize, num_values: usize, rng: &mut R) -> Expr {
    if rng.gen::<f64>() < 0.5 {
        Var(generate_random_var(num_global_variables, rng))
    } else {
        Num(generate_random_value(num_values, rng))
    }
}

fn generate_random_var<R: RngCore + ?Sized>(num_global_variables: usize, rng: &mut R) -> String {
    format!("x{}", rng.gen_range(0..num_global_variables))
}

fn generate_random_value<R: RngCore + ?Sized>(num_values: usize, rng: &mut R) -> u32 {
    rng.gen_range(0..num_values) as u32
}

/// Generates a lamport program with the given number of threads.
pub fn generate_lamport_program(num_threads: usize) -> Program {
    if num_threads < 2 {
        panic!("Number of threads must be at least 2.")
    }

    // Init
    let mut global_vars = vec!["x".to_string(), "y".to_string()];
    let mut init = vec![
        Init::Assign("x".to_string(), Num(0)),                                                       // let x: u32 = 0;
        Init::Assign("y".to_string(), Num(0)),                                                       // let y: u32 = 0;
    ];
    for i in 0..num_threads {
        global_vars.push(format!("b{i}"));
        init.push(Init::Assign(format!("b{i}"), Num(0)));                                            // b[i] = 0;
    }

    // Threads
    let mut threads = vec![];
    for i in 0..num_threads {
        let for_loop: Vec<_> = (0..num_threads).map(|j| {                            // for (j = 0; j < num_threads; j++) {
            While(Eq(Var(format!("b{j}")), Num(1)), vec![])                                          //   while (b[j] == true) {}
        }).collect();                                                                                // }

        threads.push(Thread {
            name: format!("t{i}"),
            instructions: vec![
                Assign("i".to_string(), Num(i as u32 + 1)),                                          // i = {i + 1};
                Assign("stop".to_string(), Num(0)),                                                  // stop = false;
                While(Eq(Var("stop".to_string()), Num(0)), vec![                                     // while (stop == false) {
                    Modify(format!("b{i}"), Num(1)),                                                 //   b[i] = true;
                    Modify("x".to_string(), Var("i".to_string())),                                   //   x = i;
                    If(Neg(Box::new(Eq(Var("y".to_string()), Num(0)))), vec![                        //   if (!(y == 0)) {
                        Modify(format!("b{i}"), Num(0)),                                             //     b[i] = false;
                        While(Neg(Box::new(Eq(Var("y".to_string()), Num(0)))), vec![]),              //     while (y != 0) {}
                    ], vec![                                                                         //   } else {
                        Modify("y".to_string(), Var("i".to_string())),                               //     y = i;
                        Assign("a".to_string(), Var("x".to_string())),                               //     a = x;
                        If(Neg(Box::new(Eq(Var("a".to_string()), Var("i".to_string())))), [          //     if (a != i) {
                            vec![Modify(format!("b{i}"), Num(0))],                                   //       b[i] = false;
                            for_loop,                                                                //       {for_loop}
                            vec![If(Neg(Box::new(Eq(Var("y".to_string()), Var("i".to_string())))), vec![ //   if (y != i) {
                                While(Neg(Box::new(Eq(Var("y".to_string()), Num(0)))), vec![]),      //         while (y != 0) {}
                            ], vec![                                                                 //       } else {
                                Modify("stop".to_string(), Num(1))                                   //         stop = true;
                            ])],                                                                     //       }
                        ].concat(), vec![                                                            //     } else {
                            Modify("stop".to_string(), Num(1))                                       //       stop = true;
                        ]),                                                                          //     }
                    ]),                                                                              //   }
                ]),                                                                                  // }
                Modify("y".to_string(), Num(0)),                                                     // y = 0;
                Modify(format!("b{i}"), Num(0)),                                                     // b[0] = false;
            ]
        });
    }

    Program {
        global_vars,
        init,
        threads,
        assert: vec![],
    }
}

/// Generates a peterson program with the given number of threads.
pub fn generate_peterson_program(num_threads: usize) -> Program {
    if num_threads < 2 {
        panic!("Number of threads must be at least 2.")
    }

    // Init
    let mut global_vars = vec![];
    let mut init = vec![];
    for i in 0..num_threads {
        global_vars.push(format!("level{}", i));
        init.push(Init::Assign(format!("level{}", i), Num(0)));
    }
    for i in 0..(num_threads - 1) {
        global_vars.push(format!("lastToEnter{}", i));
        init.push(Init::Assign(format!("lastToEnter{}", i), Num(0)));
    }

    // Threads
    let mut threads = vec![];
    for i in 0..num_threads {
        let mut instructions = vec![];
        for l in 0..(num_threads - 1) {
            // level[i] = l
            instructions.push(Modify(format!("level{i}"), Num(l as u32)));
            // lastToEnter[i] = l
            instructions.push(Modify(format!("lastToEnter{l}"), Var("i".to_string())));

            // there exists k != i, such that level[k] >= l
            let exists = Neg(Box::new((0..num_threads).filter(|k| *k != i)
                .map(|k| Neg(Box::new(Leq(Num(l as u32), Var(format!("level{k}"))))))
                .reduce(|e1, e2| And(Box::new(e1), Box::new(e2)))
                .unwrap()));

            // while (lastToEnter[i] == l && exists) {}
            instructions.push(While(And(Box::new(Eq(Var(format!("lastToEnter{l}")), Num(i as u32))), Box::new(exists)), vec![]));
        }

        threads.push(Thread {
            name: format!("t{i}"),
            instructions
        });
    }

    Program {
        global_vars,
        init,
        threads,
        assert: vec![],
    }
}
