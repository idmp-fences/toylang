
use std::collections::{HashMap, HashSet};
use ast::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    UndefinedInit(Init),
    UndefinedExpr(Expr),
    UndefinedModify(Statement),
    UndefinedLogic(LogicInt),
    DuplicateThread(Thread),
    DuplicateAssign(Statement),
}

pub fn check(program: &Program) -> Result<(), Error> {
    check_init(&program.init)
        .and_then(|globals| check_threads(&program.threads, &globals))
        .and_then(|locals| check_assert(&program.assert, &locals))
}

fn check_init(init: &[Init]) -> Result<HashSet<String>, Error> {
    let mut globals = HashSet::new();

    for statement in init {
        match statement {
            Init::Assign(x, expr) => {
                match expr {
                    Expr::Num(_) => {}
                    Expr::Var(x) => {
                        if !globals.contains(x) {
                            return Err(Error::UndefinedInit(statement.clone()))
                        }
                    }
                }

                globals.insert(x.to_string());
            }
        }
    }

    Ok(globals)
}

fn check_expression(expr: &Expr, globals: &HashSet<String>, locals: &HashSet<String>) -> Result<(), Error> {
    match expr {
        Expr::Num(_) => Ok(()),
        Expr::Var(x) => (globals.contains(x) || locals.contains(x))
            .then_some(())
            .ok_or_else(|| Error::UndefinedExpr(expr.clone())),
    }
}

fn check_threads(threads: &[Thread], globals: &HashSet<String>) -> Result<HashMap<String, HashSet<String>>, Error> {
    let mut thread_ids = HashSet::new();
    let mut thread_locals = HashMap::new();
    for thread in threads {
        if thread_ids.contains(&thread.name) {
            return Err(Error::DuplicateThread(thread.clone()));
        }
        thread_ids.insert(thread.name.clone());

        let mut locals = HashSet::new();
        for statement in &thread.instructions {
            match statement {
                Statement::Assign(x, expr) => {
                    if locals.contains(x) {
                        return Err(Error::DuplicateAssign(statement.clone()));
                    }

                    match check_expression(expr, globals, &locals) {
                        Ok(()) => {}
                        Err(e) => return Err(e),
                    }

                    locals.insert(x.clone());
                }
                Statement::Modify(x, expr) => {
                    if !globals.contains(x) && !locals.contains(x) {
                        return Err(Error::UndefinedModify(statement.clone()));
                    }

                    match check_expression(expr, globals, &locals) {
                        Ok(()) => {}
                        Err(e) => return Err(e),
                    }
                }
                Statement::Fence(_) => {}
            }
        }

        thread_locals.insert(thread.name.clone(), locals);
    }

    Ok(thread_locals)
}

fn check_assert(assert: &[LogicExpr], locals: &HashMap<String, HashSet<String>>) -> Result<(), Error> {
    assert.iter().try_for_each(|logic_expr| check_logic_expr(logic_expr, locals))
}

fn check_logic_expr(logic_expr: &LogicExpr, locals: &HashMap<String, HashSet<String>>) -> Result<(), Error> {
    match logic_expr {
        LogicExpr::Neg(e) => check_logic_expr(e, locals),
        LogicExpr::And(e1, e2) => check_logic_expr(e1, locals).and(check_logic_expr(e2, locals)),
        LogicExpr::Eq(e1, e2) => check_logic_int(e1, locals).and(check_logic_int(e2, locals)),
    }
}

fn check_logic_int(logic_var: &LogicInt, locals: &HashMap<String, HashSet<String>>) -> Result<(), Error> {
    match logic_var {
        LogicInt::Num(_) => Ok(()),
        LogicInt::LogicVar(thread, variable) => {
            if !locals.contains_key(thread) {
                return Err(Error::UndefinedLogic(logic_var.clone()));
            }

            if !locals.get(thread).unwrap().contains(variable) {
                return Err(Error::UndefinedLogic(logic_var.clone()));
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_expression_undefined() {
        let expr = Expr::Var("x".to_owned());
        let globals = HashSet::new();
        let locals = HashSet::new();
        let result = check_expression(&expr, &globals, &locals);
        assert_eq!(result, Err(Error::UndefinedExpr(expr)));
    }

    #[test]
    fn check_expression_global() {
        let expr = Expr::Var("x".to_owned());
        let globals = HashSet::from(["x".to_owned()]);
        let locals = HashSet::new();
        let result = check_expression(&expr, &globals, &locals);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn check_expression_local() {
        let expr = Expr::Var("x".to_owned());
        let globals = HashSet::new();
        let locals = HashSet::from(["x".to_owned()]);
        let result = check_expression(&expr, &globals, &locals);
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn assert_logic_int_undefined_thread() {
        let logic_var = LogicInt::LogicVar("t1".to_owned(), "x".to_owned());
        let locals = HashMap::new();
        let result = check_logic_int(&logic_var, &locals);
        assert_eq!(result, Err(Error::UndefinedLogic(logic_var)));
    }

    #[test]
    fn assert_logic_int_undefined_var() {
        let logic_var = LogicInt::LogicVar("t1".to_owned(), "x".to_owned());
        let locals = HashMap::from([("t1".to_owned(), HashSet::new())]);
        let result = check_logic_int(&logic_var, &locals);
        assert_eq!(result, Err(Error::UndefinedLogic(logic_var)));
    }

    #[test]
    fn check_valid_program() {
        let program = Program {
            init: Vec::from([
                Init::Assign("x".to_owned(), Expr::Num(0)),
                Init::Assign("y".to_owned(), Expr::Num(0)),
            ]),
            threads: Vec::from([
                Thread {
                    name: "t1".to_owned(),
                    instructions: Vec::from([
                        Statement::Modify("x".to_owned(), Expr::Num(1)),
                        Statement::Fence(FenceType::WR),
                        Statement::Assign("a".to_owned(), Expr::Var("x".to_owned())),
                    ]),
                },
                Thread {
                    name: "t2".to_owned(),
                    instructions: Vec::from([
                        Statement::Modify("y".to_owned(), Expr::Num(1)),
                        Statement::Fence(FenceType::WR),
                        Statement::Assign("b".to_owned(), Expr::Var("x".to_owned())),
                    ]),
                },
            ]),
            assert: Vec::from([
                LogicExpr::Neg(
                    Box::from(LogicExpr::And(
                        Box::from(LogicExpr::Eq(LogicInt::LogicVar("t1".to_owned(), "a".to_owned()), LogicInt::Num(0))),
                        Box::from(LogicExpr::Eq(LogicInt::LogicVar("t2".to_owned(), "b".to_owned()), LogicInt::Num(0)))
                    ))
                )
            ]),
        };

        assert_eq!(check(&program), Ok(()));
    }
}
