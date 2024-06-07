use petgraph::{
    adj::EdgeIndex,
    graph::{DiGraph, NodeIndex},
    visit::{EdgeRef, VisitMap, Visitable},
};

use ast::*;

use crate::critical_cycles::{self, Architecture, CriticalCycle};

pub(crate) type ThreadId = String;
pub(crate) type MemoryId = String;

// todo: use `usize` to represent memory addresses
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Read(ThreadId, MemoryId),
    Write(ThreadId, MemoryId),
    Fence(ThreadId, Fence),
}

impl Node {
    pub fn address(&self) -> Option<&MemoryId> {
        match self {
            Node::Read(_, address) => Some(address),
            Node::Write(_, address) => Some(address),
            Node::Fence(_, _) => None,
        }
    }

    pub fn thread_name(&self) -> &ThreadId {
        match self {
            Node::Read(t, _) | Node::Write(t, _) | Node::Fence(t, _) => t,
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
    pub graph: Aeg,
}

impl From<&Program> for AbstractEventGraph {
    fn from(program: &Program) -> Self {
        AbstractEventGraph {
            graph: create_aeg(program),
        }
    }
}

impl AbstractEventGraph {
    /// Find all neighbors of a node, taking into account transitive po edges
    pub fn neighbors(&self, node: NodeIndex) -> Vec<NodeIndex> {
        let mut po_neighbors = self.transitive_po_neighbors(node);
        let mut close_non_po_neighbors: Vec<NodeIndex> = self
            .graph
            .edges(node)
            .filter_map(|edge| match edge.weight() {
                AegEdge::ProgramOrder => None,
                AegEdge::Competing => Some(edge.target()),
            })
            .collect();

        close_non_po_neighbors.append(&mut po_neighbors);
        close_non_po_neighbors
    }

    /// Find all the po neighbors of a node connected through (transitive) po edges
    fn transitive_po_neighbors(&self, node: NodeIndex) -> Vec<NodeIndex> {
        let close_po_neighbors = |node| {
            self.graph
                .edges(node)
                .filter_map(|edge| match edge.weight() {
                    AegEdge::ProgramOrder => Some(edge.target()),
                    AegEdge::Competing => None,
                })
        };

        // Use DFS as backward jump can create PO loops
        let mut stack = vec![node];
        let mut discovered = self.graph.visit_map();

        while let Some(node) = stack.pop() {
            if discovered.visit(node) {
                for succ in close_po_neighbors(node) {
                    if !discovered.is_visited(&succ) {
                        stack.push(succ);
                    }
                }
            }
        }

        discovered
            .ones()
            .map(|one| NodeIndex::from(one as u32))
            .collect()
    }

    /// Check if two nodes are connected through po+,
    /// i.e. there is a path of [AegEdge::ProgramOrder] connecting them
    pub fn is_po_connected(&self, a: NodeIndex, b: NodeIndex) -> bool {
        self.transitive_po_neighbors(a).contains(&b)
    }

    pub fn tso_critical_cycles(&self) -> Vec<CriticalCycle> {
        critical_cycles::critical_cycles(self, &Architecture::Tso)
    }

    pub fn potential_fences(&self, cycles: &[CriticalCycle]) -> Vec<EdgeIndex> {
        critical_cycles::potential_fences(self, cycles)
    }
}

pub(crate) type Aeg = DiGraph<Node, AegEdge>;

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
            handle_statement(
                &mut g,
                &mut last_node,
                &mut read_nodes,
                &mut write_nodes,
                stmt,
                program.global_vars.as_ref(),
                thread.name.clone(),
            );
        }
        thread_nodes.push((write_nodes, read_nodes));
    }

    // Calculate the cmp relations
    for (i, (write_nodes, _)) in thread_nodes.iter().enumerate() {
        for write in write_nodes {
            for (_j, (other_writes, other_reads)) in
                thread_nodes.iter().enumerate().filter(|(j, _)| *j != i)
            {
                for other_write in other_writes {
                    if g[*other_write].address() == g[*write].address() {
                        // two directed edges represent an undirected relation
                        g.update_edge(*write, *other_write, AegEdge::Competing);
                        g.update_edge(*other_write, *write, AegEdge::Competing);
                    }
                }
                for other_read in other_reads {
                    if g[*other_read].address() == g[*write].address() {
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
    thread: ThreadId,
) -> Option<Vec<NodeIndex>> {
    match stmt {
        Statement::Modify(vwrite, Expr::Num(_)) | Statement::Assign(vwrite, Expr::Num(_)) => {
            // If the variable is a global, return the write node
            if globals.contains(vwrite) {
                let lhs: NodeIndex = graph.add_node(Node::Write(thread, vwrite.clone()));
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
                let lhs = graph.add_node(Node::Write(thread.clone(), vwrite.clone()));
                let rhs = graph.add_node(Node::Read(thread, vread.clone()));
                // Add a po edge from the last node to the current node
                connect_previous(graph, last_node, lhs);
                // Add a po edge from the rhs (read) to the lhs (write)
                graph.update_edge(rhs, lhs, AegEdge::ProgramOrder);

                *last_node = vec![lhs];
                write_nodes.push(lhs);
                read_nodes.push(rhs);
                Some(vec![lhs])
            } else if globals.contains(vwrite) {
                let lhs = graph.add_node(Node::Write(thread, vwrite.clone()));
                // Add a po edge from the last node to the current node
                connect_previous(graph, last_node, lhs);
                *last_node = vec![lhs];
                write_nodes.push(lhs);
                Some(vec![lhs])
            } else if globals.contains(vread) {
                let rhs = graph.add_node(Node::Read(thread, vread.clone()));
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
            let f = graph.add_node(Node::Fence(thread, Fence::Full));
            connect_previous(graph, last_node, f);
            *last_node = vec![f];
            Some(vec![f])
        }
        Statement::Fence(_) => {
            todo!("Implement other fences")
        }
        Statement::If(cond, thn, els) => {
            let mut reads = vec![];
            handle_condition(graph, &mut reads, cond, globals, thread.clone());

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

            let mut thn_branch = last_node.clone();
            let mut first_thn = None;
            for stmt in thn {
                let f = handle_statement(
                    graph,
                    &mut thn_branch,
                    read_nodes,
                    write_nodes,
                    stmt,
                    globals,
                    thread.clone(),
                );
                if first_thn.is_none() {
                    first_thn = f;
                }
            }

            let mut first_els = None;
            for stmt in els {
                let f = handle_statement(
                    graph,
                    last_node,
                    read_nodes,
                    write_nodes,
                    stmt,
                    globals,
                    thread.clone(),
                );
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
            handle_condition(graph, &mut reads, cond, globals, thread.clone());

            // Add a po edge from the last node to the first read
            let mut first_cond = None;
            if let Some(read) = reads.first() {
                first_cond = Some(vec![*read]);
                connect_previous(graph, last_node, *read);
            }
            if let Some(read) = reads.last() {
                *last_node = vec![*read];
            }

            // Move the read nodes into the read node list
            read_nodes.append(&mut reads);

            // Store the branch node for the condition
            let branch = last_node.clone();

            let mut first_body = None;
            for stmt in body {
                let f = handle_statement(
                    graph,
                    last_node,
                    read_nodes,
                    write_nodes,
                    stmt,
                    globals,
                    thread.clone(),
                );
                if first_body.is_none() {
                    first_body = f;
                }
            }

            // Condition contains a read
            if let Some(f) = first_cond {
                // Add edges from the last node to the first
                for node in f.iter() {
                    connect_previous(graph, last_node, *node);
                }

                // Next node should connect to the end of the condition
                *last_node = branch;
                Some(f)
            }
            // Body contains a read or write operation
            else if let Some(f) = first_body {
                // Add edges from the last node of the body to the start of the body
                for node in f.iter() {
                    connect_previous(graph, last_node, *node);
                }

                // Next node should connect to the end of the body and the nodes before the while loop
                last_node.append(&mut branch.clone());
                Some(f)
            }
            // While loop is empty
            else {
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
    thread: ThreadId,
) {
    match cond {
        CondExpr::Neg(e) => handle_condition(graph, reads, e, globals, thread),
        CondExpr::And(e1, e2) => {
            handle_condition(graph, reads, e1, globals, thread.clone());
            handle_condition(graph, reads, e2, globals, thread);
        }
        CondExpr::Eq(e1, e2) => {
            handle_expression(graph, reads, e1, globals, thread.clone());
            handle_expression(graph, reads, e2, globals, thread);
        }
    }
}

fn handle_expression(
    graph: &mut Aeg,
    reads: &mut Vec<NodeIndex>,
    expr: &Expr,
    globals: &[String],
    thread: ThreadId,
) {
    match expr {
        Expr::Num(_) => (),
        Expr::Var(vread) => {
            if globals.contains(vread) {
                let node = graph.add_node(Node::Read(thread, vread.clone()));
                reads
                    .last()
                    .map(|i| graph.add_edge(*i, node, AegEdge::ProgramOrder));
                reads.push(node);
            }
        }
    }
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

    let Wy = g.add_node(Node::Write("t1".to_string(), "Wy".to_string()));
    let Rx = g.add_node(Node::Read("t1".to_string(), "Rx".to_string()));

    let Wx = g.add_node(Node::Write("t2".to_string(), "Wx".to_string()));
    let Ry = g.add_node(Node::Read("t2".to_string(), "Ry".to_string()));

    g.update_edge(Wy, Rx, AegEdge::ProgramOrder);
    g.update_edge(Wx, Ry, AegEdge::ProgramOrder);

    g.update_edge(Rx, Wx, AegEdge::Competing);
    g.update_edge(Wx, Rx, AegEdge::Competing);

    g.update_edge(Ry, Wy, AegEdge::Competing);
    g.update_edge(Wy, Ry, AegEdge::Competing);

    (g, vec![vec![Rx, Wx, Ry, Wy]])
}

#[cfg(test)]
mod tests {
    use petgraph::algo::has_path_connecting;

    use super::*;

    use parser;
    use petgraph::dot::Dot;
    use petgraph::visit::IntoNodeReferences;

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

        // There are 2 writes and 2 reads
        assert_eq!(aeg.node_count(), 4);
        // Threads have 2 po each, and there are is an undirected x-x and y-y cmp edge between the threads (each of which have 2 directed edges)
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
    fn transitivity() {
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        thread t1 {
            x = 0;
            x = 1;
            x = 2;
            x = 3;
        }
        thread t2 {
            x = 4;
            y = 5;
        }
        final {
            assert( t1.a == t2.b );
        }"#;
        let program = parser::parse(program).unwrap();
        let aeg = AbstractEventGraph::from(&program);
        dbg!(&aeg);
        let mut nodes = aeg.graph.node_indices();
        let node1 = nodes.next().unwrap();
        let node2 = nodes.next().unwrap();
        let node3 = nodes.next().unwrap();
        let node4 = nodes.next().unwrap();
        assert_eq!(dbg!(aeg.neighbors(node1)).len(), 4);
        assert_eq!(aeg.neighbors(node2).len(), 3);
        assert_eq!(aeg.neighbors(node3).len(), 2);
        assert_eq!(aeg.neighbors(node4).len(), 1);
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
    fn ifs() {
        // Fig 9. and 10. of Don't Sit on the Fence
        let program = r#"
    let x: u32 = 0;
    let y: u32 = 0;
    let z: u32 = 0;
    thread t1 {
        let a: u32 = 0;
        x = 42;
        if (1 == 1) {
            y = 1;
        } else {
            a = z;
        }
        x = 1;
    }
    thread t2 {
        let b: u32 = y;
        let c: u32 = z;
        let d: u32 = x;
    }
    final {}
    "#;

        let program = parser::parse(program).unwrap();

        let aeg = create_aeg(&program);
        dbg!(Dot::with_config(&aeg, &[]));

        assert_eq!(aeg.node_count(), 7);

        assert_eq!(
            aeg.edge_references()
                .filter(|e| matches!(e.weight(), AegEdge::ProgramOrder))
                .count(),
            6
        );

        assert_eq!(
            aeg.edge_references()
                .filter(|e| matches!(e.weight(), AegEdge::Competing))
                .count(),
            3 * 2 // doubled because cmp edges are undirected
        );

        // find the t1 nodes
        let ([wx1, wy, wx2], [rz]) = get_nodes(&aeg, "t1", &["x", "y", "x"], &["z"]);

        // ensure we have the correct structure
        assert!(aeg.contains_edge(wx1, wy));
        assert!(aeg.contains_edge(wx1, rz));
        assert!(aeg.contains_edge(wy, wx2));
        assert!(aeg.contains_edge(rz, wx2));
    }

    #[test]
    fn whiles() {
        let program = r#"
    let x: u32 = 0;
    let y: u32 = 0;
    let z: u32 = 0;
    thread t1 {
        x = 32;
        while (x == 0) {
            y = 1;
        }
        z = 1;
    }
    final {}
    "#;

        let program = parser::parse(program).unwrap();

        let aeg = create_aeg(&program);
        dbg!(Dot::with_config(&aeg, &[]));

        assert_eq!(aeg.node_count(), 4);

        assert_eq!(
            aeg.edge_references()
                .filter(|e| matches!(e.weight(), AegEdge::ProgramOrder))
                .count(),
            4
        );

        assert_eq!(
            aeg.edge_references()
                .filter(|e| matches!(e.weight(), AegEdge::Competing))
                .count(),
            0
        );

        // find the t1 nodes
        let ([wx, wy, wz], [rx]) = get_nodes(&aeg, "t1", &["x", "y", "z"], &["x"]);

        // ensure we have the correct structure
        assert!(aeg.contains_edge(wx, rx));
        assert!(aeg.contains_edge(rx, wy));
        assert!(aeg.contains_edge(wy, rx));
        assert!(aeg.contains_edge(rx, wz));
    }

    #[test]
    fn whiles_no_condition() {
        let program = r#"
    let x: u32 = 0;
    let y: u32 = 0;
    let z: u32 = 0;
    thread t1 {
        let a: u32 = 0;
        x = 32;
        while (a == 0) {
            y = 1;
            a = y;
        }
        z = 1;
    }
    final {}
    "#;

        let program = parser::parse(program).unwrap();

        let aeg = create_aeg(&program);
        dbg!(Dot::with_config(&aeg, &[]));

        assert_eq!(aeg.node_count(), 4);

        assert_eq!(
            aeg.edge_references()
                .filter(|e| matches!(e.weight(), AegEdge::ProgramOrder))
                .count(),
            5
        );

        assert_eq!(
            aeg.edge_references()
                .filter(|e| matches!(e.weight(), AegEdge::Competing))
                .count(),
            0
        );

        // find the t1 nodes
        let ([wx, wy, wz], [ry]) = get_nodes(&aeg, "t1", &["x", "y", "z"], &["y"]);

        // ensure we have the correct structure
        assert!(aeg.contains_edge(wx, wy));
        assert!(aeg.contains_edge(wy, ry));
        assert!(aeg.contains_edge(ry, wy));
        assert!(aeg.contains_edge(wx, wz));
        assert!(aeg.contains_edge(ry, wz));
    }

    fn get_nodes<const N: usize, const M: usize>(
        aeg: &Aeg,
        thread: &str,
        writes: &[&str; N],
        reads: &[&str; M],
    ) -> ([NodeIndex; N], [NodeIndex; M]) {
        let mut write_nodes = vec![];
        let mut read_nodes = vec![];

        let mut wi = 0;
        let mut ri = 0;
        for (id, node) in aeg.node_references() {
            match node {
                Node::Write(t, addr) if t == thread => {
                    if wi < writes.len() && writes[wi] == addr.as_str() {
                        write_nodes.push(id);
                        wi += 1;
                    } else {
                        panic!()
                    }
                }
                Node::Read(t, addr) if t == thread => {
                    if ri < reads.len() && reads[ri] == addr.as_str() {
                        read_nodes.push(id);
                        ri += 1;
                    } else {
                        panic!()
                    }
                }
                _ => {}
            }
        }

        (
            write_nodes.try_into().unwrap(),
            read_nodes.try_into().unwrap(),
        )
    }
}
