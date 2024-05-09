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
    let mut assert = None;
    for pair in pairs {
        // A pair is a combination of the rule which matched and a span of input
        match pair.as_rule() {
            Rule::init => {
                for inner_pair in pair.into_inner() {
                    match inner_pair.as_rule() {
                        Rule::assign => {
                            init.push(parse_statement(inner_pair));
                        }
                        _ => unreachable!(),
                    }
                }
            }
            Rule::thread => {
                let mut thread = Thread {
                    name: pair.as_str().to_string(),
                    instructions: Vec::new(),
                };
                for inner_pair in pair.into_inner() {
                    match inner_pair.as_rule() {
                        Rule::stmt => {
                            thread.instructions.push(parse_statement(inner_pair));
                        }
                        _ => unreachable!(),
                    }
                }
                threads.push(thread);
            }
            Rule::assert => {
                assert = Some(parse_logic_expr(pair));
            }
            _ => {}
        }
    }
    Ok(Program {
        init,
        threads,
        assert: assert.unwrap(),
    })
}

fn parse_logic_expr(pair: pest::iterators::Pair<Rule>) -> LogicExpr {
    todo!()
}

// Match something that is modify/assign/fence
fn parse_statement(pair: pest::iterators::Pair<Rule>) -> Statement {
    match pair.as_rule() {
        Rule::assign | Rule::modify => {
            let mut pair = pair.into_inner();
            let lhs = pair.next().unwrap();
            dbg!(&lhs.as_str());
            let rhs = pair.next().unwrap().into_inner().next().unwrap();
            let rhs = match rhs.as_rule() {
                Rule::name => VarInt::Var(rhs.as_str().to_owned()),
                Rule::num => VarInt::Num(rhs.as_str().parse().unwrap()),
                _ => unreachable!(),
            };
            Statement::Assign(lhs.as_str().to_owned(), rhs)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_assign() {
        let source = "let hello: u32 = 42;";
        let mut program = ToyParser::parse(Rule::stmt, source).unwrap();
        let stmt = parse_statement(program.next().unwrap().into_inner().next().unwrap());
        assert_eq!(stmt, Statement::Assign("hello".to_owned(), VarInt::Num(42)));
    }

    #[test]
    fn parse_modify() {
        let source = "y = 33;";
        let mut program = ToyParser::parse(Rule::stmt, source).unwrap();
        let stmt = parse_statement(program.next().unwrap().into_inner().next().unwrap());
        assert_eq!(stmt, Statement::Assign("y".to_owned(), VarInt::Num(33)));
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
