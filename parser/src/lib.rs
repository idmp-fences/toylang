use ast::*;
use pest::{error::Error, Parser};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "toy.pest"]
pub struct ToyParser;

// todo: use a better error type because clippy says it's too long
pub fn parse(source: &str) -> Result<Program, Error<Rule>> {
    let pairs = ToyParser::parse(Rule::program, source)?;
    let mut init = Vec::new();
    let mut threads = Vec::new();
    let mut assert = Vec::new();
    for pair in pairs {
        // A pair is a combination of the rule which matched and a span of input
        match pair.as_rule() {
            Rule::init => {
                for inner_pair in pair.into_inner() {
                    match inner_pair.as_rule() {
                        Rule::assign => {
                            init.push(parse_init(inner_pair));
                        }
                        _ => unreachable!(),
                    }
                }
            }
            Rule::thread => {
                let mut pair = pair.into_inner();
                let mut thread = Thread {
                    name: pair.next().unwrap().as_str().to_owned(),
                    instructions: Vec::new(),
                };
                for inner_pair in pair {
                    match inner_pair.as_rule() {
                        Rule::stmt => {
                            thread.instructions.push(parse_statement(inner_pair.into_inner().next().unwrap()));
                        }
                        _ => unreachable!(),
                    }
                }
                threads.push(thread);
            }
            Rule::r#final => {
                for inner_pair in pair.into_inner() {
                    match inner_pair.as_rule() {
                        Rule::assert => {
                            assert.push(parse_logic_expr(inner_pair));
                        }
                        _ => unreachable!(),
                    }
                }
            }
            _ => {}
        }
    }
    Ok(Program {
        init,
        threads,
        assert,
    })
}

fn parse_expression(pair: pest::iterators::Pair<Rule>) -> Expr {
    match pair.as_rule() {
        Rule::name => Expr::Var(pair.as_str().to_owned()),
        Rule::num => Expr::Num(pair.as_str().parse().unwrap()),
        _ => unreachable!()
    }
}

// Match something that is an assignment
fn parse_init(pair: pest::iterators::Pair<Rule>) -> Init {
    let mut pair = pair.into_inner();
    let lhs = pair.next().unwrap();
    let rhs = pair.next().unwrap().into_inner().next().unwrap();
    let rhs = parse_expression(rhs);
    Init::Assign(lhs.as_str().to_owned(), rhs)
}

// Match something that is modify/assign/fence
fn parse_statement(pair: pest::iterators::Pair<Rule>) -> Statement {
    match pair.as_rule() {
        Rule::assign => {
            let mut pair = pair.into_inner();
            let lhs = pair.next().unwrap();
            let rhs = pair.next().unwrap().into_inner().next().unwrap();
            let rhs = parse_expression(rhs);
            Statement::Assign(lhs.as_str().to_owned(), rhs)
        }
        Rule::modify => {
            let mut pair = pair.into_inner();
            let lhs = pair.next().unwrap();
            let rhs = pair.next().unwrap().into_inner().next().unwrap();
            let rhs = parse_expression(rhs);
            Statement::Modify(lhs.as_str().to_owned(), rhs)
        }
        Rule::fence => {
            let fence = pair.into_inner().next().unwrap();
            let fence = match fence.as_str() {
                "WR" => FenceType::WR,
                "WW" => FenceType::WW,
                "RW" => FenceType::RW,
                "RR" => FenceType::RR,
                _ => unreachable!(),
            };
            Statement::Fence(fence)
        }
        _ => unreachable!(),
    }
}

fn parse_logic_expr(pair: pest::iterators::Pair<Rule>) -> LogicExpr {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_expr_num() {
        let source = "42";
        let mut program = ToyParser::parse(Rule::expr, source).unwrap();
        let expr = parse_expression(program.next().unwrap().into_inner().next().unwrap());
        assert_eq!(expr, Expr::Num(42));
    }

    #[test]
    fn parse_expr_var() {
        let source = "x";
        let mut program = ToyParser::parse(Rule::expr, source).unwrap();
        let expr = parse_expression(program.next().unwrap().into_inner().next().unwrap());
        assert_eq!(expr, Expr::Var("x".to_owned()));
    }

    #[test]
    fn parse_init_test() {
        let source = "let hello: u32 = 42;";
        let mut program = ToyParser::parse(Rule::init, source).unwrap();
        let init = parse_init(program.next().unwrap().into_inner().next().unwrap());
        assert_eq!(init, Init::Assign("hello".to_owned(), Expr::Num(42)));
    }

    #[test]
    fn parse_assign() {
        let source = "let hello: u32 = 42;";
        let mut program = ToyParser::parse(Rule::stmt, source).unwrap();
        let stmt = parse_statement(program.next().unwrap().into_inner().next().unwrap());
        assert_eq!(stmt, Statement::Assign("hello".to_owned(), Expr::Num(42)));
    }

    #[test]
    fn parse_modify() {
        let source = "y = 33;";
        let mut program = ToyParser::parse(Rule::stmt, source).unwrap();
        let stmt = parse_statement(program.next().unwrap().into_inner().next().unwrap());
        assert_eq!(stmt, Statement::Assign("y".to_owned(), Expr::Num(33)));
    }

    #[test]
    fn parse_fence() {
        let source = "Fence(WR);";
        let mut program = ToyParser::parse(Rule::stmt, source).unwrap();
        let stmt = parse_statement(program.next().unwrap().into_inner().next().unwrap());
        assert_eq!(stmt, Statement::Fence(FenceType::WR));
    }

    #[test]
    fn parse_program() {
        let source = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        thread t1 {
            x = 1;
            Fence(WR);
            let a: u32 = x;
        }
        thread t2 {
            y = 1;
            Fence(WR);
            let b: u32 = x;
        }
        final {
            assert( !( t1.a == 0 && t2.b == 0 ) );
        }
        "#;
        let program = parse(source).unwrap();
        dbg!(program);
    }
}
