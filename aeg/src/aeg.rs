use petgraph::{
    algo::tarjan_scc,
    graph::{DiGraph, NodeIndex},
};

use ast::*;

use crate::dfs::ProgramOrderDfs;

// todo: use `usize` to represent memory addresses
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Read(String),
    Write(String),
    Fence(Fence),
}

impl Node {
    pub fn name(&self) -> Option<&Name> {
        match self {
            Node::Read(address) => Some(address),
            Node::Write(address) => Some(address),
            Node::Fence(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AegEdge {
    /// Abstracts all po edges that connect two events in program order.
    /// Note that this does not include po+, the transitive edges. For example, the relation
    /// a --> b --> c is represented by two edges: a --> b and b --> c, and the edge a --> c is implied.
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

#[derive(Debug, Clone)]
pub struct AbstractEventGraph {
    graph: Aeg,
}

impl From<&Program> for AbstractEventGraph {
    fn from(program: &Program) -> Self {
        AbstractEventGraph {
            graph: create_aeg(program),
        }
    }
}

impl AbstractEventGraph {
    pub fn critical_cycles(&self) -> Vec<Vec<NodeIndex>> {
        critical_cycles(&self.graph)
    }

    pub fn potential_fences(&self) -> Vec<Fence> {
        todo!()
    }
}

pub(crate) type Aeg = DiGraph<Node, AegEdge>;

// todo: We only consider accesses to shared variables and ignore the local variables. Does this mean local variables are not part of the AEG?
// also, can we ignore fences
fn create_aeg(program: &Program) -> Aeg {
    let mut g: Aeg = DiGraph::new();

    // The init block is single-threaded, so none of the nodes are in the AEG.
    // All competing edges happen between threads.

    // Add the threads
    let mut thread_nodes = vec![];
    for thread in &program.threads {
        let mut last_node = vec![];
        let mut read_nodes = vec![];
        let mut write_nodes = vec![];
        for stmt in &thread.instructions {
            handle_statement(&mut g, &mut last_node, &mut read_nodes, &mut write_nodes, stmt, program.global_vars.as_ref());
        }
        thread_nodes.push((write_nodes, read_nodes));
    }
    dbg!(&thread_nodes);

    // Add the transitive po edges
    for node in g.node_indices() {
        let mut dfs = ProgramOrderDfs::new(&g, node);
        dfs.next(&g);
        while let Some(next) = dfs.next(&g) {
            g.update_edge(node, next, AegEdge::ProgramOrder);
        }
    }

    // Calculate the cmp relations
    for (i, (write_nodes, _)) in thread_nodes.iter().enumerate() {
        for write in write_nodes {
            for (_j, (other_writes, other_reads)) in
            thread_nodes.iter().enumerate().filter(|(j, _)| *j != i)
            {
                for other_write in other_writes {
                    if g[*other_write].name() == g[*write].name() {
                        // two directed edges represent an undirected relation
                        g.update_edge(*write, *other_write, AegEdge::Competing);
                        g.update_edge(*other_write, *write, AegEdge::Competing);
                    }
                }
                for other_read in other_reads {
                    if g[*other_read].name() == g[*write].name() {
                        g.update_edge(*write, *other_read, AegEdge::Competing);
                        g.update_edge(*other_read, *write, AegEdge::Competing);
                    }
                }
            }
        }
    }
    g
}

/// Adds the corresponding nodes for a statement to the AEG and returns the index of the first nodes.
/// Only the global read/write nodes are added to the AEG as they are the only ones that can create competing edges.
/// The local read/write nodes are not add to the AEG as they are not relevant for the competing edge calculation.
fn handle_statement(
    graph: &mut Aeg,
    last_node: &mut Vec<NodeIndex>,
    read_nodes: &mut Vec<NodeIndex>,
    write_nodes: &mut Vec<NodeIndex>,
    stmt: &Statement,
    globals: &[String],
) -> Option<Vec<NodeIndex>> {
    match stmt {
        Statement::Modify(vwrite, Expr::Num(_)) | Statement::Assign(vwrite, Expr::Num(_)) => {
            // If the variable is a global, return the write node
            if globals.contains(vwrite) {
                let lhs: NodeIndex = graph.add_node(Node::Write(vwrite.clone()));
                // Add a po edge from the last node to the current node
                connect_previous(graph, last_node, lhs);
                *last_node = vec![lhs];

                write_nodes.push(lhs);
                Some(vec![lhs])
            } else {
                None
            }
        }
        Statement::Modify(vwrite, Expr::Var(vread))
        | Statement::Assign(vwrite, Expr::Var(vread)) => {
            // We distinguish between 4 cases, whether both are globals, only one is a global, or none are globals

            if globals.contains(vwrite) && globals.contains(vread) {
                let lhs = graph.add_node(Node::Write(vwrite.clone()));
                let rhs = graph.add_node(Node::Read(vread.clone()));
                // Add a po edge from the last node to the current node
                connect_previous(graph, last_node, lhs);
                // Add a po edge from the rhs (read) to the lhs (write)
                graph.update_edge(rhs, lhs, AegEdge::ProgramOrder);

                *last_node = vec![lhs];
                write_nodes.push(lhs);
                read_nodes.push(rhs);
                Some(vec![lhs])
            } else if globals.contains(vwrite) {
                let lhs = graph.add_node(Node::Write(vwrite.clone()));
                // Add a po edge from the last node to the current node
                connect_previous(graph, last_node, lhs);
                *last_node = vec![lhs];
                write_nodes.push(lhs);
                Some(vec![lhs])
            } else if globals.contains(vread) {
                let rhs = graph.add_node(Node::Read(vread.clone()));
                // Add a po edge from the last node to the current node
                connect_previous(graph, last_node, rhs);
                *last_node = vec![rhs];
                read_nodes.push(rhs);
                Some(vec![rhs])
            } else {
                None
            }
        }
        Statement::Fence(FenceType::WR) => {
            // Fences are always part of the AEG as they affect the critical cycles
            let f = graph.add_node(Node::Fence(Fence::Full));
            connect_previous(graph, last_node, f);
            *last_node = vec![f];
            Some(vec![f])
        }
        Statement::Fence(_) => {
            todo!("Implement other fences")
        }
        Statement::If(cond, thn, els) => {
            let mut reads = vec![];
            handle_condition(graph, &mut reads, cond, globals);

            // Add a po edge from the last node to the first read
            let mut first = None;
            if let Some(read) = reads.first() {
                first = Some(vec![*read]);
                connect_previous(graph, last_node, *read);
            }
            if let Some(read) = reads.last() {
                *last_node = vec![*read];
            }

            // Move the read nodes into the read node list
            read_nodes.append(&mut reads);

            let branch_node = last_node.clone();
            let mut first_thn = None;
            for stmt in thn {
                let f = handle_statement(graph, last_node, read_nodes, write_nodes, stmt, globals);
                if first_thn.is_none() {
                    first_thn = f;
                }
            }

            let mut thn_branch = last_node.clone();
            let mut first_els = None;
            *last_node = branch_node;
            for stmt in els {
                let f = handle_statement(graph, &mut thn_branch, read_nodes, write_nodes, stmt, globals);
                if first_els.is_none() {
                    first_els = f;
                }
            }

            for node in thn_branch.iter() {
                if !last_node.contains(node) {
                    last_node.push(*node);
                }
            }
            
            if first.is_some() {
                first
            } else if let Some(mut ft) = first_thn {
                if let Some(fe) = first_els {
                    ft.extend(fe);
                }
                Some(ft)
            } else {
                first_els
            }
        }
        Statement::While(cond, body) => {
            let mut reads = vec![];
            handle_condition(graph, &mut reads, cond, globals);

            // Add a po edge from the last node to the first read
            let mut first = None;
            if let Some(read) = reads.first() {
                first = Some(vec![*read]);
                connect_previous(graph, last_node, *read);
            }
            if let Some(read) = reads.last() {
                *last_node = vec![*read];
            }

            // Move the read nodes into the read node list
            read_nodes.append(&mut reads);

            for stmt in body {
                let f = handle_statement(graph, last_node, read_nodes, write_nodes, stmt, globals);
                if first.is_none() {
                    first = f;
                }
            }

            // Add edges from the last node to the first
            if let Some(f) = first {
                for node in f.iter() {
                    connect_previous(graph, last_node, *node);
                }
                
                // Next node should connect to the first node
                last_node.clone_from(&f);
                Some(f)
            } else {
                None
            }
        }
    }
}

fn connect_previous(graph: &mut Aeg, last_node: &[NodeIndex], current_node: NodeIndex) {
    for node in last_node {
        graph.update_edge(*node, current_node, AegEdge::ProgramOrder);
    }
}

fn handle_condition(
    graph: &mut Aeg,
    reads: &mut Vec<NodeIndex>,
    cond: &CondExpr,
    globals: &[String],
) {
    match cond {
        CondExpr::Neg(e) => handle_condition(graph, reads, e, globals),
        CondExpr::And(e1, e2) => {
            handle_condition(graph, reads, e1, globals);
            handle_condition(graph, reads, e2, globals);
        }
        CondExpr::Eq(e1, e2) => {
            handle_expression(graph, reads, e1, globals);
            handle_expression(graph, reads, e2, globals);
        }
    }
}

fn handle_expression(graph: &mut Aeg, reads: &mut Vec<NodeIndex>, expr: &Expr, globals: &[String]) {
    match expr {
        Expr::Num(_) => (),
        Expr::Var(vread) => {
            if globals.contains(vread) {
                let node = graph.add_node(Node::Read(vread.clone()));
                reads.last().map(|i| graph.add_edge(*i, node, AegEdge::ProgramOrder));
                reads.push(node);
            }
        }
    }
}

// todo: critical cycles must be minimal
// (CS1) the cycle contains at least one delay for A;
// (CS2) per thread, there are at most two accesses, the accesses are adjacent in the
// cycle, and the accesses are to different memory locations; and
// (CS3) for a memory location l, there are at most three accesses to l along the cycle,
// the accesses are adjacent in the cycle, and the accesses are from different threads.
fn critical_cycles(g: &Aeg) -> Vec<Vec<NodeIndex>> {
    let tarjan = tarjan_scc(&g);

    dbg!(&tarjan);

    tarjan
        .iter()
        .filter(|cycle| is_critical(g, cycle))
        .cloned()
        .collect()
}

/// Returns the potential critical cycles for the following aeg:
///
/// ```text
/// Wy --\   /-- Wx
/// |     \ /    |
/// |      x     |
/// v     / \    v
/// Rx --/   \-- Ry
/// ```
///
/// I *believe* the optimal solution is placing on one of the two directed edges.
///
/// Which is the aeg generated by the following program:
///
/// ```text
/// let x: u32 = 0;
/// let y: u32 = 0;
/// thread t1 {
///     x = 1;
///     let a: u32 = y;
/// }
/// thread t2 {
///     y = 1;
///     let b: u32 = x;
/// }
/// final {
///     assert( t1.a == t2.b );
/// }
/// ```
#[deprecated(note = "This function is temporary and will be removed")]
#[allow(non_snake_case)]
fn dummy_critical_cycles() -> (Aeg, Vec<Vec<NodeIndex>>) {
    let mut g: Aeg = Aeg::new();

    let Wy = g.add_node(Node::Write("Wy".to_string()));
    let Rx = g.add_node(Node::Read("Rx".to_string()));

    let Wx = g.add_node(Node::Write("Wx".to_string()));
    let Ry = g.add_node(Node::Read("Ry".to_string()));

    g.update_edge(Wy, Rx, AegEdge::ProgramOrder);
    g.update_edge(Wx, Ry, AegEdge::ProgramOrder);

    g.update_edge(Rx, Wx, AegEdge::Competing);
    g.update_edge(Wx, Rx, AegEdge::Competing);

    g.update_edge(Ry, Wy, AegEdge::Competing);
    g.update_edge(Wy, Ry, AegEdge::Competing);

    (g, vec![vec![Rx, Wx, Ry, Wy]])
}

// a delay is a po or rf edge that is not safe (i.e., is relaxed) for a given architecture
fn is_critical(g: &Aeg, scc: &[NodeIndex]) -> bool {
    // The order of node ids within each cycle returned by tarjan_scc is arbitrary.
    // So we check all pairs of nodes in the cycle for competing edges.
    false
}

#[cfg(test)]
mod tests {
    use petgraph::algo::has_path_connecting;
    use petgraph::dot::Dot;

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
            global_vars: vec!["x".to_string(), "y".to_string(), "z".to_string()],
        };

        let aeg = create_aeg(&program);
        assert_eq!(aeg.node_count(), 0);
        assert_eq!(aeg.edge_count(), 0);
    }

    #[test]
    fn aeg_from_threads() {
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        thread t1 {
            x = 1;
            let a: u32 = y;
        }
        thread t2 {
            y = 1;
            let b: u32 = x;
        }
        final {
            assert( t1.a == t2.b );
        }"#;
        let program = parser::parse(program).unwrap();

        let aeg = create_aeg(&program);

        dbg!("{:?}", Dot::with_config(&aeg, &[]));

        // There are 6 assignments, so 6 nodes in the aeg
        assert_eq!(aeg.node_count(), 4);
        // Init has 1 po, threads have 2 po each, and there are is an undirected x-x and y-y cmp edge between the threads (each of which have 2 directed edges)
        assert_eq!(aeg.edge_count(), 6);

        // There should be a cycle
        let n = aeg.node_indices().collect::<Vec<_>>();
        assert!(has_path_connecting(
            &aeg,
            *n.first().unwrap(),
            *n.last().unwrap(),
            None
        ));
    }

    #[test]
    fn critical_cycle() {
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        thread t1 {
            x = 1;
            let a: u32 = y;
        }
        thread t2 {
            y = 1;
            let b: u32 = x;
        }
        final {
            // the following is possible under tso
            assert( !(t1.a == 0 && t1.b == 0) );
        }"#;
        let program = parser::parse(program).unwrap();

        let aeg = create_aeg(&program);
        dbg!(&aeg);

        println!("{:?}", Dot::with_config(&aeg, &[]));

        // Calculate the critical cycles
        let ccs = critical_cycles(&aeg);
        dbg!(&ccs);
        assert_eq!(ccs.len(), 1);
    }

    #[test]
    fn competing_edges() {
        // make sure competing edges don't appear within read-read pairs, or within local variables from different threads
        let program = r#"
        let x: u32 = 3;
        thread t1 {
            let a: u32 = x;
        }
        thread t2 {
            let a: u32 = x;
        }
        final {
            assert( t1.a == t2.a );
        }"#;
        let program = parser::parse(program).unwrap();

        let aeg = dbg!(create_aeg(&program));
        assert_eq!(aeg.node_count(), 2);
        assert_eq!(aeg.edge_count(), 0);

        let program = r#"
        let x: u32 = 3;
        thread t1 {
            let a: u32 = x;
            x = 5;
        }
        thread t2 {
            let a: u32 = x;
        }
        final {
            assert( t1.a == t2.a );
        }"#;
        let program = parser::parse(program).unwrap();

        let aeg = dbg!(create_aeg(&program));
        assert_eq!(aeg.node_count(), 3);
        // 1 program order, 1 undirected competing edge
        assert_eq!(aeg.edge_count(), 3);

        let program = r#"
        let x: u32 = 3;
        thread t1 {
            let a: u32 = x;
            let b: u32 = x;
        }
        thread t2 {
            let a: u32 = x;
        }
        final {
            assert( t1.a == t2.a );
        }"#;
        let program = parser::parse(program).unwrap();

        let aeg = dbg!(create_aeg(&program));
        assert_eq!(aeg.node_count(), 3);
        // 1 program order
        assert_eq!(aeg.edge_count(), 1);
    }

    #[test]
    fn dont_sit_fig_16() {
        let program = r#"
        let y: u32 = 0;
        let z: u32 = 0;
        let t: u32 = 0;
        thread t1 {
            let x: u32 = 0;
            x = t;
            x = y;
        }
        thread t2 {
            y = 1;
            z = 1;
            t = 1;
        }
        thread t3 {
            let x: u32 = 0;
            x = z;
            x = y;
        }
        thread t4 {
            let x: u32 = 0;
            x = t;
            x = z;
        }
        final {
            assert( 0 == 0 );
        }
        "#;
        let ast = parser::parse(program).unwrap();
        let aeg = AbstractEventGraph::from(&ast);
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));
        let ccs = aeg.critical_cycles();
        dbg!(&ccs);
        assert_eq!(ccs.len(), 3);
    }
}
