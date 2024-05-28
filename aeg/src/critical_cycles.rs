use std::collections::HashMap;

use petgraph::{
    graph::NodeIndex,
    visit::{VisitMap, Visitable},
};

use crate::aeg::{Aeg, AegEdge, MemoryId, Node, ThreadId};

#[derive(Clone, Debug)]
pub struct CriticalCycle {
    pub cycle: Vec<NodeIndex>,
}

/// A struct representing a (possibly incomplete) minimal cycle in the AEG.
///
/// A minimal cycle is a cycle that satisfies the following properties:
///
/// MC1: Per thread, there are at most two accesses, and the accesses are adjacent in the cycle.
///
/// MC2: For a memory location l, there are at most three accesses to l along the cycle, and the accesses are adjacent in the cycle.
#[derive(Clone, Debug)]
struct IncompleteMinimalCycle<T, M>
where
    T: Eq + std::hash::Hash,
    M: Eq + std::hash::Hash,
{
    /// The nodes in the cycle.
    cycle: Vec<NodeIndex>,
    thread_accesses: HashMap<T, usize>,
    memory_accesses: HashMap<M, usize>,
    has_delay: bool,
}

impl IncompleteMinimalCycle<ThreadId, MemoryId> {
    fn new() -> Self {
        IncompleteMinimalCycle {
            cycle: Vec::new(),
            thread_accesses: HashMap::new(),
            memory_accesses: HashMap::new(),
            has_delay: false,
        }
    }

    fn len(&self) -> usize {
        self.cycle.len()
    }

    fn first(&self) -> Option<&NodeIndex> {
        self.cycle.first()
    }

    fn last(&self) -> Option<&NodeIndex> {
        self.cycle.last()
    }

    /// Add a node to the cycle if it satisfies the minimal cycle properties.
    /// Returns true if the node was added to the cycle.
    ///
    /// Panics if the added node is not a successor of the last node in the cycle.
    fn add_node(&mut self, graph: &Aeg, node: NodeIndex) -> bool {
        if self.cycle.contains(&node) {
            return false;
        }

        let (thread_accesses, memory_accesses) = match &graph[node] {
            Node::Write(thread, addr) | Node::Read(thread, addr) => (
                self.thread_accesses.entry(thread.clone()).or_insert(0),
                self.memory_accesses.entry(addr.clone()).or_insert(0),
            ),

            Node::Fence(_, _) => {
                unimplemented!("should fences even be part of the AEG?")
            }
        };

        // MC1: Per thread, there are at most two accesses,
        // and the accesses are adjacent in the cycle
        let mc1 = match thread_accesses {
            0 => true,
            1 => {
                if *self.cycle.last().unwrap() != node {
                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        // MC2: For a memory location l, there are at most three accesses to l along the cycle,
        // and the accesses are adjacent in the cycle
        let mc2 = match memory_accesses {
            0 => true,
            1 | 2 => {
                if *self.cycle.last().unwrap() != node {
                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        // some micro-optimizations could be made in this function for less hash lookups
        if !(mc1 && mc2) {
            return false;
        }

        *thread_accesses += 1;
        *memory_accesses += 1;

        if let Some(last) = self.last() {
            let last_edge = graph
                .find_edge(*last, node)
                .expect("node added is not a successor of the last in the cycle");

            // mark cycle as critical if there is poWR edge
            match (&graph[last_edge], &graph[*last], &graph[node]) {
                (AegEdge::ProgramOrder, _, _) => {
                    self.has_delay = true;
                }
                _ => {}
            }
        }

        self.cycle.push(node);

        true
    }

    pub fn make_critical(self) -> Option<CriticalCycle> {
        if self.has_delay {
            Some(CriticalCycle { cycle: self.cycle })
        } else {
            None
        }
    }
}

/// Find all critical cycles in an Aeg
pub fn critical_cycles(graph: &Aeg) -> Vec<CriticalCycle> {
    let mut all_cycles = Vec::new();
    let mut inner_cycles = Vec::new();

    // DFS state
    let mut stack = Vec::new();
    let mut discovered = graph.visit_map();

    // Nodes for which critical cycles have been found
    let mut explored = Vec::new();

    // Go through all nodes in the graph, starting a DFS from each node to find critical cycles
    for start_node in graph.node_indices() {
        // Reset the state of the DFS
        stack.clear();
        discovered.clear();
        let mut mc = IncompleteMinimalCycle::new();
        debug_assert!(mc.add_node(graph, start_node));
        stack.push(mc);

        while let Some(cycle) = stack.pop() {
            let node = *cycle.last().expect("cycle is empty");

            if discovered.visit(node) {
                for succ in graph.neighbors(node) {
                    if !explored.contains(&succ) {
                        let mut cycle = cycle.clone();
                        if cycle.add_node(graph, succ) {
                            stack.push(cycle);
                        } else if *cycle.first().unwrap() == succ && cycle.len() > 2 {
                            if let Some(cycle) = cycle.make_critical() {
                                inner_cycles.push(cycle);
                            }
                        }
                    }
                }
            }
        }

        all_cycles.append(&mut inner_cycles);
        explored.push(start_node);
    }
    all_cycles
}
#[cfg(test)]
mod test {
    use petgraph::dot::Dot;

    use super::*;
    use crate::aeg::{AbstractEventGraph, AegEdge};

    #[test]
    #[allow(non_snake_case)]
    fn minimal_cycle() {
        let mut g = Aeg::new();

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

        let mut mc = IncompleteMinimalCycle::new();
        assert!(mc.add_node(&g, Wy));
        assert!(mc.add_node(&g, Rx));

        // can't add Wy again
        assert!(!mc.add_node(&g, Wy));
    }

    #[test]
    #[should_panic(expected = "node added is not a successor of the last in the cycle")]
    #[allow(non_snake_case)]
    fn minimal_cycle_panics() {
        let mut g = Aeg::new();

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

        let mut mc = IncompleteMinimalCycle::new();
        assert!(mc.add_node(&g, Wy));
        assert!(mc.add_node(&g, Rx));

        // panics because Ry is not a successor of Rx
        mc.add_node(&g, Ry);
    }

    #[test]
    fn simple_critical_cycle() {
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
        let ast = parser::parse(program).unwrap();
        let aeg = AbstractEventGraph::from(&ast);
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));

        let ccs = critical_cycles(&aeg.graph);
        assert_eq!(ccs.len(), 1);
    }

    #[test]
    fn dont_sit_fig_16() {
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        let z: u32 = 0;
        let t: u32 = 0;
        thread t1 {
            t = 1;
            y = 1;
        }
        thread t2 {
            let a: u32 = z;
            x = 2;
        }
        thread t3 {
            let a: u32 = x;
            let b: u32 = y;
            z = 3;
            let c: u32 = t;
        }
        thread t4 {
            let a: u32 = z;
            y = 4;
        }
        thread t5 {
            t = 5;
            let a: u32 = z;
        }
        final {
            assert( 0 == 0 );
        }
        "#;
        let ast = parser::parse(program).unwrap();
        let aeg = AbstractEventGraph::from(&ast);
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));

        let ccs = critical_cycles(&aeg.graph);
        dbg!(&ccs);
        assert_eq!(ccs.len(), 6);
    }
}
