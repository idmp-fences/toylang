use std::collections::HashMap;

use petgraph::{
    graph::NodeIndex,
    prelude::EdgeIndex,
    visit::{VisitMap, Visitable},
};

use crate::{
    aeg::{MemoryId, Node, ThreadId},
    AbstractEventGraph,
};

/// A critical cycle that satisfies the properties in *Don't sit on the Fence*.
#[derive(Clone, Debug)]
pub struct CriticalCycle {
    cycle: Vec<NodeIndex>,
    potential_fences: Vec<EdgeIndex>,
}

#[derive(Clone, Copy, Debug)]
pub enum Architecture {
    Tso,
    Arm,
    Power,
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
    // could be a shortvec (size 2 and s3 respectively)
    thread_accesses: HashMap<T, Vec<usize>>,
    memory_accesses: HashMap<M, Vec<usize>>,
    has_delay: bool,
    architecture: Architecture,
}

impl IncompleteMinimalCycle<ThreadId, MemoryId> {
    fn new_tso() -> Self {
        Self::with_architecture(&Architecture::Tso)
    }

    fn with_architecture(architecture: &Architecture) -> Self {
        IncompleteMinimalCycle {
            cycle: Vec::new(),
            thread_accesses: HashMap::new(),
            memory_accesses: HashMap::new(),
            has_delay: false,
            architecture: *architecture,
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
    fn add_node(&mut self, aeg: &AbstractEventGraph, node: NodeIndex) -> bool {
        if self.cycle.contains(&node) {
            return false;
        }

        let (thread, addr) = match &aeg.graph[node] {
            Node::Write(thread, addr) | Node::Read(thread, addr) => (thread, addr),

            Node::Fence(_, _) => {
                unimplemented!("should fences even be part of the AEG?")
            }
        };

        let thread_accesses = self.thread_accesses.entry(thread.clone()).or_insert(vec![]);
        let memory_accesses = self.memory_accesses.entry(addr.clone()).or_insert(vec![]);

        // MC1: Per thread, there are at most two accesses,
        // and the accesses are adjacent in the cycle
        let mc1 = match thread_accesses.len() {
            0 => true,
            1 => {
                let last = &aeg.graph[*self.cycle.last().unwrap()];
                let first = &aeg.graph[*self.cycle.first().unwrap()];
                last.thread_name() == thread || first.thread_name() == thread
            } // return true only if this is an adjacent access
            _ => false,
        };

        // MC2: For a memory location l, there are at most three accesses to l along the cycle,
        // and the accesses are adjacent in the cycle
        let mc2 = match memory_accesses.len() {
            0 => true,
            1 | 2 => {
                let last = &aeg.graph[*self.cycle.last().unwrap()];
                let first = &aeg.graph[*self.cycle.first().unwrap()];
                last.address().unwrap() == addr || first.address().unwrap() == addr
            } // return true only if this is an adjacent access
            _ => false,
        };

        // some micro-optimizations could be made in this function for less hash lookups
        if !(mc1 && mc2) {
            return false;
        }

        thread_accesses.push(self.cycle.len());
        memory_accesses.push(self.cycle.len());

        // a delay is a po or rf edge that is not safe (i.e., is relaxed) for a given architecture
        if let Some(last) = self.last() {
            debug_assert!(
                aeg.neighbors(*last).contains(&node),
                "node added is not a successor of the last in the cycle"
            );

            if aeg.is_po_connected(*last, node) {
                // mark cycle as critical if there is poWR edge
                match (&self.architecture, &aeg.graph[*last], &aeg.graph[node]) {
                    (Architecture::Power, _, _) => {
                        self.has_delay = true;
                    }
                    (Architecture::Tso, Node::Write(_, _), Node::Read(_, _)) => {
                        self.has_delay = true;
                    }
                    (Architecture::Tso, _, _) => {}
                    (Architecture::Arm, _, _) => {
                        unimplemented!("Delay not defined for {:?}", self.architecture)
                    }
                }
            }
        }

        self.cycle.push(node);

        true
    }

    /// Turn this cycle into a critical cycle if it has a delay and satisfies adjacent thread/memory properties, otherwise return None.
    /// It is up to the caller to ensure that [IncompleteMinimalCycle::cycle] forms a cycle in the AEG.
    ///
    /// Note that we need to check again for thread/memory properties because of the cyclical nature.
    /// For example, a variable `t1.x` could be added to the following incomplete cycle:
    ///
    /// `t1.y --> t2.a --> t2.b --> ...`
    ///
    /// Because the second access to thread t1 is adjacent to the first node in the cycle.
    /// Later on, however, a different variable `t3.c` could be added as such:
    ///
    /// `t1.y --> t2.a --> t2.b --> t1.x --> t3.c`
    ///
    /// because it is not prevented by [IncompleteMinimalCycle::add_node].
    /// The reason it is not prevented is because we could have the following cycle:
    ///
    /// `t1.a --> t2.b --> t2.a --> t3.a`
    ///
    /// where, after the addition of `t2.a`, the addition of `tn.a` should still be possible.
    ///
    /// TODO: Really, it should be possible to add this check in the [IncompleteMinimalCycle::add_node] function,
    /// and not have to go through the cycle again. Worth looking into if this becomes a bottleneck.
    fn complete(self, aeg: &AbstractEventGraph) -> Option<CriticalCycle> {
        if self.has_delay {
            for (_, nodes) in &self.thread_accesses {
                debug_assert!(nodes.len() <= 2);
                if nodes.len() == 2 {
                    let idx1 = nodes[0];
                    let idx2 = nodes[1];
                    if !((idx2 - idx1) == 1 || (idx2 - idx1) == self.cycle.len() - 1) {
                        return None;
                    }
                }
            }
            for (_, nodes) in &self.memory_accesses {
                debug_assert!(nodes.len() <= 3);
                if nodes.len() == 2 {
                    let idx1 = nodes[0];
                    let idx2 = nodes[1];
                    if !((idx2 - idx1) == 1 || (idx2 - idx1) == self.cycle.len() - 1) {
                        return None;
                    }
                }
                if nodes.len() == 3 {
                    let idx1 = nodes[0];
                    let idx2 = nodes[1];
                    let idx3 = nodes[2];

                    if !((idx2 - idx1) == 1 || (idx2 - idx1) == self.cycle.len() - 1) {
                        return None;
                    }
                    if !((idx3 - idx2) == 1 || (idx3 - idx2) == self.cycle.len() - 1) {
                        return None;
                    }
                }
            }

            let pf = potential_fences(aeg, &self.cycle, &self.architecture);
            Some(CriticalCycle {
                cycle: self.cycle,
                potential_fences: pf,
            })
        } else {
            None
        }
    }
}

/// Find all critical cycles in an [AbstractEventGraph] for the given architecture
pub(crate) fn critical_cycles(
    aeg: &AbstractEventGraph,
    architecture: &Architecture,
) -> Vec<CriticalCycle> {
    // We go through each of the nodes in the abstract event graph, using DFS on each to look for critical cycles

    let mut all_cycles = Vec::new();
    let mut inner_cycles = Vec::new();

    // DFS state, reset at each node
    let mut stack = Vec::new();
    let mut discovered = aeg.graph.visit_map();

    // Nodes for which critical cycles have been found.
    let mut explored = Vec::new();

    // Go through all nodes in the graph, starting a DFS from each node to find critical cycles
    for start_node in aeg.graph.node_indices() {
        // Reset the state of the DFS
        stack.clear();
        discovered.clear();
        let mut mc = IncompleteMinimalCycle::with_architecture(architecture);
        debug_assert!(mc.add_node(aeg, start_node));
        stack.push(mc);

        while let Some(cycle) = stack.pop() {
            let node = *cycle.last().expect("cycle is empty");

            if discovered.visit(node) {
                for succ in aeg.neighbors(node) {
                    if !explored.contains(&succ) {
                        let mut cycle = cycle.clone();
                        if cycle.add_node(aeg, succ) {
                            stack.push(cycle);
                        } else if *cycle.first().unwrap() == succ && cycle.len() > 2 {
                            if let Some(cycle) = cycle.complete(aeg) {
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

/// Find all potential fence placements in a cycle.
///
/// Under TSO, this equates to all poWR edges in the critical cycle.
/// However, this edge could be in po+ (a transitive edge) thus we must
/// list all the po edges between the two nodes.
///
/// TODO: handle the case where the write and read sandwich an if/else statement.
/// Currently it will just pick any shortest of the branching po paths and return those edges,
/// which I don't think is correct, as placing a fence on just one of the branches will still
/// allow critical cycles through the other branch.
fn potential_fences(
    aeg: &AbstractEventGraph,
    cycle: &[NodeIndex],
    architecture: &Architecture,
) -> Vec<EdgeIndex> {
    let n = cycle.len();
    let mut potential_fences = vec![];
    for i in 0..n {
        let j = {
            if i < n - 1 {
                i + 1
            } else {
                0
            }
        };

        let u = cycle[i];
        let v = cycle[j];

        if let Some(path) = aeg.po_between(u, v) {
            // There is a po(+) edge between u and v
            match (architecture, &aeg.graph[u], &aeg.graph[v]) {
                // Only alow poWR edges on TSO
                (Architecture::Tso, Node::Write(_, _), Node::Read(_, _)) => {}
                (Architecture::Tso, _, _) => continue,
                (Architecture::Arm, _, _) => unimplemented!(),
                // Allow everything on Power
                (Architecture::Power, _, _) => {}
            }

            potential_fences.extend(
                path.windows(2)
                    .map(|pair| aeg.graph.find_edge(pair[0], pair[1]).unwrap()),
            )
        }
    }
    potential_fences
}

#[cfg(test)]
mod test {
    use petgraph::dot::Dot;

    use super::*;
    use crate::aeg::{AbstractEventGraph, Aeg, AegEdge};

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

        let aeg = AbstractEventGraph { graph: g };

        let mut mc = IncompleteMinimalCycle::new_tso();
        assert!(mc.add_node(&aeg, Wy));
        assert!(mc.add_node(&aeg, Rx));

        // can't add Wy again
        assert!(!mc.add_node(&aeg, Wy));
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

        let aeg = AbstractEventGraph { graph: g };

        let mut mc = IncompleteMinimalCycle::new_tso();
        assert!(mc.add_node(&aeg, Wy));
        assert!(mc.add_node(&aeg, Rx));

        // panics because Ry is not a successor of Rx
        mc.add_node(&aeg, Ry);
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

        let ccs = critical_cycles(&aeg, &Architecture::Power);
        assert_eq!(ccs.len(), 1);

        let ccs = critical_cycles(&aeg, &Architecture::Tso);
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

        let ccs = critical_cycles(&aeg, &Architecture::Power);
        dbg!(&ccs);
        assert_eq!(ccs.len(), 4);
        assert_eq!(
            ccs.iter().flat_map(|cc| cc.potential_fences.iter()).count(),
            3 + 3 + 2 + 2
        );

        let ccs = critical_cycles(&aeg, &Architecture::Tso);
        dbg!(&ccs);
        assert_eq!(ccs.len(), 1);
        assert_eq!(ccs[0].potential_fences.len(), 2)
    }

    // This panics because fences aren't implemented into the AEG yet.
    // I'm not sure if fences should be part of the aeg (as they do in the paper), or
    // as a special type of edge (which would be easier to work with imo)
    #[test]
    fn fenced_program() {
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        thread t1 {
            x = 1;
            Fence(WR);
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

        let ccs = critical_cycles(&aeg, &Architecture::Power);
        assert_eq!(ccs.len(), 0);
    }
}
