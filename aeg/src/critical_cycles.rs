use std::collections::HashMap;

use petgraph::{
    graph::NodeIndex,
    visit::{VisitMap, Visitable},
};

use crate::aeg::{Aeg, MemoryId, Node, ThreadId};

/// A struct representing a (possibly incomplete) minimal cycle in the AEG.
///
/// A minimal cycle is a cycle that satisfies the following properties:
///
/// MC1: Per thread, there are at most two accesses, and the accesses are adjacent in the cycle.
///
/// MC2: For a memory location l, there are at most three accesses to l along the cycle, and the accesses are adjacent in the cycle.
#[derive(Clone, Debug)]
struct MinimalCycle<T, M>
where
    T: Eq + std::hash::Hash,
    M: Eq + std::hash::Hash,
{
    nodes: Vec<NodeIndex>,
    thread_accesses: HashMap<T, usize>,
    memory_accesses: HashMap<M, usize>,
}

impl MinimalCycle<ThreadId, MemoryId> {
    fn new() -> Self {
        MinimalCycle {
            nodes: Vec::new(),
            thread_accesses: HashMap::new(),
            memory_accesses: HashMap::new(),
        }
    }

    fn first(&self) -> Option<&NodeIndex> {
        self.nodes.first()
    }

    fn last(&self) -> Option<&NodeIndex> {
        self.nodes.last()
    }

    /// Add a node to the cycle if it satisfies the minimal cycle properties
    /// Returns true if the node was added to the cycle
    fn add_node(&mut self, graph: &Aeg, node: NodeIndex) -> bool {
        if self.nodes.contains(&node) {
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
                if *self.nodes.last().unwrap() != node {
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
                if *self.nodes.last().unwrap() != node {
                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        // some micro-optimizations could be made in this function for less hash lookups
        if mc1 && mc2 {
            self.nodes.push(node);
            *thread_accesses += 1;
            *memory_accesses += 1;
            true
        } else {
            false
        }
    }
}

/// A modified version of [petgraph::visit::Dfs],
#[derive(Clone, Debug)]
pub struct CriticalCycleDFS {
    /// The stack of nodes for which critical cycles have been found
    pub explored: Vec<NodeIndex>,
    /// The stack of cycles to visit
    pub stack: Vec<MinimalCycle<String, String>>,
    /// The map of discovered nodes
    pub discovered: <Aeg as Visitable>::Map,
}

impl Default for CriticalCycleDFS {
    fn default() -> Self {
        CriticalCycleDFS {
            explored: Vec::new(),
            stack: Vec::new(),
            discovered: <Aeg as Visitable>::Map::default(),
        }
    }
}

impl CriticalCycleDFS {
    /// Create a new **CriticalCycleDFS**, using the graph's visitor map, and put **start**
    /// in the stack of nodes to visit.
    pub fn new(graph: &Aeg, start: NodeIndex) -> Self {
        let mut dfs = CriticalCycleDFS::empty(graph);
        dfs.explored.clear();
        dfs.explored.push(start);
        dfs
    }

    /// Create a new **CriticalCycleDFS** using the graph's visitor map, and no stack.
    pub fn empty(graph: &Aeg) -> Self {
        CriticalCycleDFS {
            explored: Vec::new(),
            stack: Vec::new(),
            discovered: graph.visit_map(),
        }
    }

    /// Return the next critical cycle
    pub fn all_cycles(&mut self, graph: &Aeg) -> Vec<MinimalCycle<String, String>> {
        let mut all_cycles = vec![];
        let mut cycles = vec![];

        for start_node in graph.node_indices() {
            // Reset the state of the DFS
            self.stack.clear();
            let mut mc = MinimalCycle::new();
            debug_assert!(mc.add_node(graph, start_node));
            self.stack.push(mc);
            self.discovered = graph.visit_map();

            while let Some(cycle) = self.stack.pop() {
                let node = *cycle.last().expect("cycle is empty");

                if self.discovered.visit(node) {
                    for succ in graph.neighbors(node) {
                        if
                        /* !self.discovered.is_visited(&succ) && */
                        !self.explored.contains(&succ)
                        // && matches!(
                        //     graph[graph.find_edge(node, succ).unwrap()],
                        //     AegEdge::ProgramOrder
                        // )
                        {
                            let mut cycle = cycle.clone();
                            if cycle.add_node(graph, succ) {
                                self.stack.push(cycle);
                            } else if *cycle.first().unwrap() == succ && cycle.nodes.len() > 2 {
                                // if the cycle contains the successor and is longer than 2
                                // (i.e. it's a cycle and not a single node)
                                // and the successor is not in the explored set (i.e. it's not a cycle we've already found
                                // return the cycle
                                cycles.push(cycle.clone());
                            }
                        } else {
                        }
                    }
                }
            }

            all_cycles.append(&mut cycles);
            self.explored.push(start_node);
        }
        all_cycles
    }
}

pub fn find_critical_cycles(graph: &Aeg) -> Vec<MinimalCycle<String, String>> {
    let mut dfs = CriticalCycleDFS::empty(graph);
    let cycles = dfs.all_cycles(graph);
    for cycle in &cycles {
        dbg!(&cycle.nodes);
    }
    cycles
}

#[cfg(test)]
mod test {
    use petgraph::dot::Dot;

    use super::*;
    use crate::aeg::{AbstractEventGraph, AegEdge};

    #[test]
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

        let mut mc = MinimalCycle::new();
        assert!(mc.add_node(&g, Wy));
        assert!(mc.add_node(&g, Rx));

        assert!(!mc.add_node(&g, Wy));

        assert!(mc.add_node(&g, Ry));
        assert!(mc.add_node(&g, Wx));
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

        let ccs = find_critical_cycles(&aeg.graph);
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

        let ccs = find_critical_cycles(&aeg.graph);
        dbg!(&ccs);
        assert_eq!(ccs.len(), 4);
    }
}
