use ast::*;
use petgraph::{
    adj::EdgeIndex,
    algo::astar,
    graph::{DiGraph, Edge, NodeIndex},
    visit::EdgeRef,
};

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

    fn transitive_po_neighbors(&self, node: NodeIndex) -> Vec<NodeIndex> {
        // find all the po neighbors of this node, and all the po neighbors of them
        let close_po_neighbors = self
            .graph
            .edges(node)
            .filter_map(|edge| match edge.weight() {
                AegEdge::ProgramOrder => Some(edge.target()),
                AegEdge::Competing => None,
            });

        let mut neighbors: Vec<NodeIndex> = close_po_neighbors.clone().collect();

        for n in close_po_neighbors {
            neighbors.append(&mut self.transitive_po_neighbors(n))
        }

        neighbors
    }

    /// Check if two nodes are connected through po+,
    /// i.e. there is a path of [AegEdge::ProgramOrder] connecting them
    pub fn is_po_connected(&self, a: NodeIndex, b: NodeIndex) -> bool {
        self.transitive_po_neighbors(a).contains(&b)
    }

    /// Returns the shortest program order path between two nodes, if it exists
    pub fn po_between(&self, a: NodeIndex, b: NodeIndex) -> Option<Vec<NodeIndex>> {
        if !self.is_po_connected(a, b) {
            return None;
        }

        astar(
            &self.graph,
            a,
            |finish| finish == b,
            |edge| match edge.weight() {
                AegEdge::ProgramOrder => 0,
                AegEdge::Competing => 100,
            },
            |_| 0,
        )
        .map(|(_cost, path)| path)
    }

    pub fn tso_critical_cycles(&self) -> Vec<CriticalCycle> {
        critical_cycles::critical_cycles(self, &Architecture::Tso)
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
        let mut last_node = None;
        let mut read_nodes = vec![];
        let mut write_nodes = vec![];
        for stmt in &thread.instructions {
            let (write, read) = handle_statement(
                &mut g,
                &mut last_node,
                stmt,
                program.global_vars.as_ref(),
                thread.name.clone(),
            );
            if let Some(node) = write {
                write_nodes.push(node);
            }
            if let Some(node) = read {
                read_nodes.push(node);
            }
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

/// Adds the corresponding nodes for a statement to the AEG, and returns the (write, read) nodes.
/// Only the global read/write nodes are returned as they are the only ones that can create competing edges.
/// The local read/write nodes are not returned as they are not relevant for the competing edge calculation.
fn handle_statement(
    graph: &mut Aeg,
    last_node: &mut Option<NodeIndex>,
    stmt: &Statement,
    globals: &[String],
    thread: ThreadId,
) -> (Option<NodeIndex>, Option<NodeIndex>) {
    match stmt {
        Statement::Modify(vwrite, Expr::Num(_)) | Statement::Assign(vwrite, Expr::Num(_)) => {
            // If the variable is a global, return the write node
            if globals.contains(vwrite) {
                let lhs: NodeIndex = graph.add_node(Node::Write(thread, vwrite.clone()));
                // Add a po edge from the last node to the current node
                if last_node.is_some() {
                    graph.update_edge(last_node.unwrap(), lhs, AegEdge::ProgramOrder);
                }
                *last_node = Some(lhs);

                (Some(lhs), None)
            } else {
                (None, None)
            }
        }
        Statement::Modify(vwrite, Expr::Var(vread))
        | Statement::Assign(vwrite, Expr::Var(vread)) => {
            // We distinguish between 4 cases, wether both are globals, only one is a global, or none are globals

            if globals.contains(vwrite) && globals.contains(vread) {
                let lhs = graph.add_node(Node::Write(thread.clone(), vwrite.clone()));
                let rhs = graph.add_node(Node::Read(thread, vread.clone()));
                // Add a po edge from the last node to the current node
                if last_node.is_some() {
                    graph.update_edge(last_node.unwrap(), rhs, AegEdge::ProgramOrder);
                }
                // Add a po edge from the rhs (read) to the lhs (write)
                graph.update_edge(rhs, lhs, AegEdge::ProgramOrder);

                *last_node = Some(lhs);
                (Some(lhs), Some(rhs))
            } else if globals.contains(vwrite) {
                let lhs = graph.add_node(Node::Write(thread, vwrite.clone()));
                // Add a po edge from the last node to the current node
                if last_node.is_some() {
                    graph.update_edge(last_node.unwrap(), lhs, AegEdge::ProgramOrder);
                }
                *last_node = Some(lhs);
                (Some(lhs), None)
            } else if globals.contains(vread) {
                let rhs = graph.add_node(Node::Read(thread, vread.clone()));
                // Add a po edge from the last node to the current node
                if last_node.is_some() {
                    graph.update_edge(last_node.unwrap(), rhs, AegEdge::ProgramOrder);
                }
                *last_node = Some(rhs);
                (None, Some(rhs))
            } else {
                (None, None)
            }
        }
        Statement::Fence(FenceType::WR) => {
            // Fences are always part of the AEG as they affect the critical cycles
            let f = graph.add_node(Node::Fence(thread, Fence::Full));
            if last_node.is_some() {
                graph.update_edge(last_node.unwrap(), f, AegEdge::ProgramOrder);
            }
            *last_node = Some(f);
            (None, None)
        }
        Statement::Fence(_) => {
            todo!("Implement other fences")
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

    return (g, vec![vec![Rx, Wx, Ry, Wy]]);
}

#[cfg(test)]
mod tests {
    use petgraph::algo::has_path_connecting;

    use super::*;

    use parser;
    use petgraph::dot::Dot;

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
        assert!(aeg.node_count() == 2);
        assert!(aeg.edge_count() == 0);

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
        assert!(aeg.node_count() == 3);
        // 1 program order, 1 undirected competing edge
        assert!(aeg.edge_count() == 3);

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
        assert!(aeg.node_count() == 3);
        // 1 program order
        assert!(aeg.edge_count() == 1);
    }
}
