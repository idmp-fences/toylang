use ast::*;
use petgraph::{
    algo::tarjan_scc,
    graph::{DiGraph, NodeIndex},
};

// todo: use `usize` to represent memory addresses
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Read(String),
    Write(String),
    Fence(Fence),
}

// pub enum Edge {
//     /// Total order of events in the same thread
//     ProgramOrder,
//     ///
//     FromRead,
//     /// Links a write event e1 to a read event e2 such that e2 reads the value written by e1
//     ReadFrom,
//     /// Total order of writes to the same address, also known as the write serialization
//     Coherence,
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AegEdge {
    /// Abstracts all po edges that connect two events in program order.
    ProgramOrder,
    /// External communications coe rfe fre are overapproximated by this relation.
    /// Internal communications are already covered by transitivity of [AegEdge::ProgramOrder].
    Competing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fence {
    /// mfence in x86, sync in Power, dmb in ARM
    Full,
    /// lwsync in Power
    LightWeight,
    /// isync in Power, isb in ARM
    Control,
}

pub type Aeg = DiGraph<Node, AegEdge>;
pub fn create_aeg(program: &Program) -> Aeg {
    let mut g = DiGraph::new();
    let mut last_node: Option<NodeIndex> = None;

    // Start with the init block
    for init in &program.init {
        let stmt = Statement::from(init.clone());
        handle_statment(&mut g, &mut last_node, &stmt);
    }

    let last_init_node = last_node.unwrap();

    // Add the threads
    let mut thread_nodes = vec![];
    for thread in &program.threads {
        let mut last_node = Some(last_init_node);
        let mut read_nodes = vec![];
        let mut write_nodes = vec![];
        for stmt in &thread.instructions {
            let (write, read) = handle_statment(&mut g, &mut last_node, stmt);
            if write.is_some() {
                write_nodes.push(write.unwrap());
            }
            if read.is_some() {
                read_nodes.push(read.unwrap());
            }
        }
        thread_nodes.push((write_nodes, read_nodes));
    }

    // Calculate the cmp relations
    for (i, (write_nodes, _)) in thread_nodes.iter().enumerate() {
        for write in write_nodes {
            for (other_writes, other_reads) in &thread_nodes[i + 1..] {
                for other_write in other_writes {
                    if g[*other_write] == g[*write] {
                        // two directed edges represent an undirected relation
                        g.add_edge(*write, *other_write, AegEdge::Competing);
                        g.add_edge(*other_write, *write, AegEdge::Competing);
                    }
                }
                for other_read in other_reads {
                    if g[*other_read] == g[*write] {
                        g.add_edge(*write, *other_read, AegEdge::Competing);
                        g.add_edge(*other_read, *write, AegEdge::Competing);
                    }
                }
            }
        }
    }
    g
}

/// Adds the corresponding nodes for a statement to the AEG, and returns the (write, read) nodes.
/// todo: only return global read/write nodes as they are the only ones that can conflict.
fn handle_statment(
    graph: &mut Aeg,
    last_node: &mut Option<NodeIndex>,
    stmt: &Statement,
) -> (Option<NodeIndex>, Option<NodeIndex>) {
    match stmt {
        Statement::Modify(var, Expr::Num(_)) | Statement::Assign(var, Expr::Num(_)) => {
            let lhs: NodeIndex = graph.add_node(Node::Write(var.clone()));
            if last_node.is_some() {
                graph.add_edge(last_node.unwrap(), lhs, AegEdge::ProgramOrder);
            }
            *last_node = Some(lhs);
            (Some(lhs), None)
        }
        Statement::Modify(vwrite, Expr::Var(vread))
        | Statement::Assign(vwrite, Expr::Var(vread)) => {
            let rhs = graph.add_node(Node::Read(vread.clone()));

            // In toy, the lhs is always a write.
            // This is different from the procedure described in DSotF,
            // where the lhs can be also be a read.
            // For example `*(y+x) = 1;` is valid in C but not in toy.
            let lhs = graph.add_node(Node::Write(vwrite.clone()));
            if last_node.is_some() {
                graph.add_edge(last_node.unwrap(), rhs, AegEdge::ProgramOrder);
            }
            graph.add_edge(rhs, lhs, AegEdge::ProgramOrder);
            *last_node = Some(lhs);
            (Some(lhs), Some(rhs))
        }
        Statement::Fence(FenceType::WR) => {
            let f = graph.add_node(Node::Fence(Fence::Full));
            if last_node.is_some() {
                graph.add_edge(last_node.unwrap(), f, AegEdge::ProgramOrder);
            }
            *last_node = Some(f);
            (None, None)
        }
        Statement::Fence(_) => {
            todo!("Implement other fences")
        }
    }
}

pub fn critical_cycles(g: &Aeg) -> Vec<Vec<NodeIndex>> {
    let tarjan = tarjan_scc(&g);

    tarjan
        .iter()
        .filter(|cycle| is_critical(g, cycle))
        .map(|cycle| cycle.clone())
        .collect()
}

fn is_critical(g: &Aeg, scc: &[NodeIndex]) -> bool {
    // The order of node ids within each cycle returned by tarjan_scc is arbitrary.
    // So we check all pairs of nodes in the cycle for competing edges.
    for (i, n1) in scc.iter().enumerate() {
        for n2 in &scc[i + 1..] {
            // For a competing edge, there must be an edge from n1 to n2 (and from n2 to n1)
            if let Some(edge) = g.edges_connecting(*n1, *n2).next() {
                if *edge.weight() == AegEdge::Competing {
                    return true;
                }
            }
        }
    }
    return false;
}

#[cfg(test)]
mod tests {
    use petgraph::algo::has_path_connecting;

    use super::*;

    #[test]
    fn aeg_from_init() {
        let program = Program {
            init: vec![
                Init::Assign("x".to_string(), Expr::Num(1)),
                Init::Assign("y".to_string(), Expr::Num(2)),
                Init::Assign("z".to_string(), Expr::Var("x".to_string())),
            ],
            threads: vec![],
            assert: vec![LogicExpr::Eq(LogicInt::Num(1), LogicInt::Num(1))],
        };

        let aeg = create_aeg(&program);
        assert_eq!(aeg.node_count(), 4);
        assert_eq!(aeg.edge_count(), 3);
        let n = aeg.node_indices().collect::<Vec<_>>();
        assert!(has_path_connecting(
            &aeg,
            *n.first().unwrap(),
            *n.last().unwrap(),
            None
        ));
    }

    #[test]
    fn aeg_from_threads() {
        let program = Program {
            init: vec![
                Init::Assign("x".to_string(), Expr::Num(0)),
                Init::Assign("y".to_string(), Expr::Num(0)),
            ],
            threads: vec![
                Thread {
                    name: "t1".to_string(),
                    instructions: vec![
                        Statement::Assign("x".to_string(), Expr::Num(1)),
                        Statement::Assign("y".to_string(), Expr::Num(0)),
                    ],
                },
                Thread {
                    name: "t2".to_string(),
                    instructions: vec![
                        Statement::Assign("y".to_string(), Expr::Num(1)),
                        Statement::Assign("x".to_string(), Expr::Num(0)),
                    ],
                },
            ],
            assert: vec![LogicExpr::Eq(LogicInt::Num(1), LogicInt::Num(1))],
        };

        let aeg = create_aeg(&program);
        dbg!(&aeg);

        // There are 6 assignments, so 6 nodes in the aeg
        assert_eq!(aeg.node_count(), 6);
        // Init has 1 po, threads have 2 po each, and there are is an undirected x-x and y-y cmp edge between the threads (each of which have 2 directed edges)
        assert_eq!(aeg.edge_count(), 9);
        let n = aeg.node_indices().collect::<Vec<_>>();
        assert!(has_path_connecting(
            &aeg,
            *n.first().unwrap(),
            *n.last().unwrap(),
            None
        ));

        // Calculate the critical cycles
        let ccs = critical_cycles(&aeg);
        dbg!(&ccs);
        assert_eq!(ccs.len(), 1);
    }

    #[test]
    fn competing_edges() {
        // make sure competing edges don't appear within read-read pairs, or within local variables from different threads
        let program = Program {
            init: vec![Init::Assign("x".to_string(), Expr::Num(3))],
            threads: vec![
                Thread {
                    name: "t1".to_string(),
                    instructions: vec![Statement::Assign(
                        "a".to_string(),
                        Expr::Var("x".to_string()),
                    )],
                },
                Thread {
                    name: "t2".to_string(),
                    instructions: vec![Statement::Assign(
                        "a".to_string(),
                        Expr::Var("x".to_string()),
                    )],
                },
            ],
            assert: vec![LogicExpr::Eq(
                LogicInt::LogicVar("t1".to_string(), "a".to_string()),
                LogicInt::LogicVar("t2".to_string(), "a".to_string()),
            )],
        };

        let aeg = dbg!(create_aeg(&program));
        let ccs = dbg!(critical_cycles(&aeg));
        assert_eq!(ccs.len(), 0);
    }
}
