use arrayvec::ArrayVec;
use hashbrown::HashMap;
use hashbrown::HashSet;

use itertools::Itertools;
use petgraph::{graph::NodeIndex, prelude::EdgeIndex};
use std::hash::Hash;

use crate::aeg::AegEdge;
use crate::{
    aeg::{MemoryId, Node, ThreadId},
    simple_paths::all_simple_po_paths,
    AbstractEventGraph,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A critical cycle that satisfies the properties in *Don't sit on the Fence*.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CriticalCycle {
    pub cycle: Vec<NodeIndex>,
    pub potential_fences: Vec<EdgeIndex>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Architecture {
    Tso,
    Arm,
    Power,
}

/// A struct representing a (possibly incomplete) critical cycle in the AEG.
///
/// A critical cycle is a cycle that satisfies the following properties:
///
/// (CS1) the cycle contains at least one delay for A;
/// (CS2) per thread, there are at most two accesses, the accesses are adjacent in the
/// cycle, and the accesses are to different memory locations; and
/// (CS3) for a memory location l, there are at most three accesses to l along the cycle,
/// the accesses are adjacent in the cycle, and the accesses are from different threads.
///
/// An incomplete minimal cycle will satisfy CS2 and CS3. Delays are checked when calling [IncompleteCriticalCycle::complete]
#[derive(Clone, Debug)]
struct IncompleteCriticalCycle<T, M>
where
    T: Eq + std::hash::Hash,
    M: Eq + std::hash::Hash,
{
    /// The nodes in the cycle.
    cycle: Vec<NodeIndex>,
    // could be a shortvec (size 2 and s3 respectively)
    thread_accesses: HashMap<T, ArrayVec<usize, 2>>,
    memory_accesses: HashMap<M, ArrayVec<usize, 3>>,
    architecture: Architecture,
}

impl Hash for IncompleteCriticalCycle<ThreadId, MemoryId> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.cycle.hash(state);
    }
}

impl PartialEq for IncompleteCriticalCycle<ThreadId, MemoryId> {
    fn eq(&self, other: &Self) -> bool {
        self.cycle == other.cycle && self.architecture == other.architecture
    }
}

impl Eq for IncompleteCriticalCycle<ThreadId, MemoryId> {}

impl IncompleteCriticalCycle<ThreadId, MemoryId> {
    #[inline]
    fn with_architecture(architecture: Architecture) -> Self {
        IncompleteCriticalCycle {
            cycle: Vec::new(),
            thread_accesses: HashMap::new(),
            memory_accesses: HashMap::new(),
            architecture,
        }
    }

    #[inline]
    fn from_cycle(cycle: Vec<NodeIndex>, aeg: &AbstractEventGraph) -> Self {
        // Construct the thread accesses
        let mut thread_accesses = HashMap::new();
        let mut memory_accesses = HashMap::new();
        for (i, node) in cycle.iter().enumerate() {
            let (thread_id, memory_id) = match aeg.graph[*node] {
                Node::Read(t, m) | Node::Write(t, m) => (t, m),
                Node::Fence(_, _) => unreachable!(),
            };
            thread_accesses
                .entry(thread_id)
                .and_modify(|v: &mut ArrayVec<usize, 2>| v.push(i))
                .or_insert({
                    let mut av = ArrayVec::new();
                    av.push(i);
                    av
                });
            memory_accesses
                .entry(memory_id)
                .and_modify(|v: &mut ArrayVec<usize, 3>| v.push(i))
                .or_insert({
                    let mut av = ArrayVec::new();
                    av.push(i);
                    av
                });
        }

        IncompleteCriticalCycle {
            cycle,
            thread_accesses,
            memory_accesses,
            architecture: aeg.config.architecture,
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.cycle.len()
    }

    #[inline]
    fn first(&self) -> Option<&NodeIndex> {
        self.cycle.first()
    }

    #[inline]
    fn last(&self) -> Option<&NodeIndex> {
        self.cycle.last()
    }

    #[inline]
    fn forms_thread_cycle(&self, aeg: &AbstractEventGraph) -> bool {
        if self.cycle.len() < 3 {
            false
        } else {
            let last = &aeg.graph[*self.last().unwrap()];
            let first = &aeg.graph[*self.first().unwrap()];
            last.thread_name() == first.thread_name()
        }
    }

    #[inline]
    fn forms_addr_cycle(&self, aeg: &AbstractEventGraph) -> bool {
        if self.cycle.len() < 4 {
            false
        } else {
            let last = &aeg.graph[*self.last().unwrap()];
            let first = &aeg.graph[*self.first().unwrap()];
            last.address().unwrap() == first.address().unwrap()
        }
    }

    /// Check if a node can be added into the AEG while satisfying minimal cycle properties
    ///
    /// For example, a variable `t1.x` could be added to the following incomplete cycle:
    ///
    /// `t1.y --> t2.a --> t2.b --> ...`
    ///
    /// Because the second access to thread t1 is adjacent to the first node in the cycle.
    /// When a different variable `t3.c` wants to be added to the resulting cycle,
    ///
    /// `t1.y --> t2.a --> t2.b --> t1.x --> ...`
    ///
    /// This is now prevented because it would break CS2 as the accesses in t1 are no longer adjacent.
    /// In fact, in this case no more addition is possible.
    ///
    /// In the case below:
    ///
    /// `t1.x --> t1.y --> t1.x --> t2.x --> ...`
    ///
    /// only the addition of an event `tn.x`  is possible with n different from 1 or 2.
    ///
    /// TODO: add more rigorous tests
    /// TODO: perf: if no more addition is possible, we want to complete the cycle early?
    /// TODO: perf: hashmaps could be hashsets
    fn can_add_node(&self, aeg: &AbstractEventGraph, node: NodeIndex) -> bool {
        debug_assert!(
            {
                if let Some(last) = self.last() {
                    aeg.neighbors(*last).any(|neighbor| neighbor == node)
                } else {
                    true
                }
            },
            "node added is not a successor of the last in the cycle"
        );

        if self.cycle.contains(&node) {
            return false;
        }

        let (thread, addr) = match &aeg.graph[node] {
            Node::Write(thread, addr) | Node::Read(thread, addr) => (thread, addr),

            Node::Fence(_, _) => {
                unimplemented!("should fences even be part of the AEG?")
            }
        };

        if self.forms_thread_cycle(aeg) {
            return false;
        }
        if self.forms_addr_cycle(aeg) {
            let last = &aeg.graph[*self.last().unwrap()];
            if thread != last.address().unwrap() {
                return false;
            }
        }

        let thread_accesses = self.thread_accesses.get(thread);
        let memory_accesses = self.memory_accesses.get(addr);

        // CS2: Per thread, there are at most two accesses,
        // and the accesses are adjacent in the cycle
        // and the accesses are to different memory locations
        let cs2 = thread_accesses.is_none()
            || match thread_accesses.unwrap().len() {
                0 => true,
                1 => {
                    let last = &aeg.graph[*self.cycle.last().unwrap()];
                    if last.thread_name() == thread && last.address().unwrap() != addr {
                        true
                    } else {
                        let first = &aeg.graph[*self.cycle.first().unwrap()];
                        first.thread_name() == thread && first.address().unwrap() != addr
                    }
                } // return true only if this is an adjacent access and the access is to a different memory location
                _ => false,
            };

        if !cs2 {
            return false;
        }

        // MC2: For a memory location l, there are at most three accesses to l along the cycle,
        // and the accesses are adjacent in the cycle
        // and the accesses are from different threads
        let cs3 = memory_accesses.is_none()
            || match memory_accesses.unwrap().len() {
                0 => true,
                1 | 2 => {
                    let is_same_thread = memory_accesses
                        .unwrap()
                        .iter()
                        .map(|i| self.cycle[*i])
                        .any(|node| aeg.graph[node].thread_name() == thread);
                    if is_same_thread {
                        false
                    } else {
                        let last = &aeg.graph[*self.cycle.last().unwrap()];
                        if last.address().unwrap() == addr {
                            true
                        } else {
                            let first = &aeg.graph[*self.cycle.first().unwrap()];
                            first.address().unwrap() == addr
                        }
                    }
                } // return true only if this is an adjacent access and the access is from a different thread
                _ => false,
            };

        cs2 && cs3
    }

    fn add_node_unchecked(&mut self, aeg: &AbstractEventGraph, node: NodeIndex) {
        debug_assert!(self.can_add_node(aeg, node));
        let (thread, addr) = match &aeg.graph[node] {
            Node::Write(thread, addr) | Node::Read(thread, addr) => (thread, addr),

            Node::Fence(_, _) => {
                unimplemented!("should fences even be part of the AEG?")
            }
        };

        let thread_accesses = self.thread_accesses.entry(*thread).or_default();
        let memory_accesses = self.memory_accesses.entry(*addr).or_default();

        thread_accesses.push(self.cycle.len());
        memory_accesses.push(self.cycle.len());

        self.cycle.push(node);
    }

    /// Add a node to the cycle if it satisfies the minimal cycle properties.
    /// Returns true if the node was added to the cycle.
    ///
    /// Panics on debug if the added node is not a successor of the last node in the cycle.
    fn add_node(&mut self, aeg: &AbstractEventGraph, node: NodeIndex) -> bool {
        if !self.can_add_node(aeg, node) {
            return false;
        }

        self.add_node_unchecked(aeg, node);
        true
    }

    #[inline]
    fn _edge_has_delay(
        &self,
        aeg: &AbstractEventGraph,
        node1: NodeIndex,
        node2: NodeIndex,
    ) -> bool {
        // PERF: we can assume node1 and node2 are connected by a transitive po edge if they are NOT connected with a cmp edge
        // so instead of aeg.is_po_connected(node1, node2) we do the following
        if !aeg
            .graph
            .edges_connecting(node1, node2)
            .any(|e| *e.weight() == AegEdge::Competing)
        {
            debug_assert!(aeg.is_po_connected(node1, node2));
            // mark cycle as critical if there is poWR edge
            match (&self.architecture, &aeg.graph[node1], &aeg.graph[node2]) {
                (Architecture::Power, _, _) => {
                    return true;
                }
                (Architecture::Tso, Node::Write(_, _), Node::Read(_, _)) => {
                    return true;
                }
                (Architecture::Tso, _, _) => {}
                (Architecture::Arm, _, _) => {
                    unimplemented!("Delay not defined for {:?}", self.architecture)
                }
            }
        }
        false
    }

    #[inline]
    fn has_delay(&self, aeg: &AbstractEventGraph) -> bool {
        for events in self.cycle.windows(2) {
            if self._edge_has_delay(aeg, events[0], events[1]) {
                return true;
            }
        }
        if let (Some(&first), Some(&last)) = (self.first(), self.last()) {
            if self._edge_has_delay(aeg, last, first) {
                return true;
            }
        }
        false
    }

    /// Turn this cycle into a critical cycle if it has a delay and satisfies adjacent thread/memory properties, otherwise return None.
    /// It is up to the caller to ensure that [IncompleteCriticalCycle::cycle] forms a cycle in the AEG.
    ///
    /// Note that [IncompleteCriticalCycle::add_node] ensures that the adjacent thread/memory properties are upheld.
    fn complete(self, aeg: &AbstractEventGraph) -> Vec<CriticalCycle> {
        if !self.has_delay(aeg) {
            return vec![];
        }

        let pfs = potential_fences(aeg, &self.cycle);

        pfs.into_iter()
            .map(|pf| CriticalCycle {
                cycle: self.cycle.clone(),
                potential_fences: pf,
            })
            .collect()
    }
}

/// Find all critical cycles in an [AbstractEventGraph] for the given architecture
pub(crate) fn critical_cycles(aeg: &AbstractEventGraph) -> Vec<CriticalCycle> {
    // We go through each of the nodes in the abstract event graph, using DFS on each to look for critical cycles

    let mut all_cycles = Vec::new();
    let mut inner_cycles = Vec::new();

    // DFS state, reset at each node
    let mut stack = Vec::new();
    let mut discovered = HashSet::with_capacity(aeg.graph.node_count());

    // Nodes for which all critical cycles have been found.
    let mut explored = HashSet::with_capacity(aeg.graph.node_count());

    // Go through all nodes in the graph, starting a DFS from each node to find critical cycles
    for start_node in aeg.graph.node_indices() {
        // Reset the state of the DFS
        stack.clear();
        discovered.clear();
        let mut mc = IncompleteCriticalCycle::with_architecture(aeg.config.architecture);
        let was_added = mc.add_node(aeg, start_node);
        debug_assert!(was_added);
        stack.push(mc);

        while let Some(cycle) = stack.pop() {
            let node = *cycle.last().expect("cycle is empty");

            if discovered.insert(cycle.clone()) {
                for succ in aeg.neighbors(node) {
                    if !explored.contains(&succ) {
                        // PERF: check if node can be added, and only then clone
                        if cycle.can_add_node(aeg, succ) {
                            let mut cycle = cycle.clone();
                            cycle.add_node_unchecked(aeg, succ);
                            stack.push(cycle);
                        } else if *cycle.first().unwrap() == succ && cycle.len() > 2 {
                            inner_cycles.extend(cycle.clone().complete(aeg));
                        }
                    }
                }
            }
        }

        all_cycles.append(&mut inner_cycles);
        explored.insert(start_node);
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
fn potential_fences(aeg: &AbstractEventGraph, cycle: &[NodeIndex]) -> Vec<Vec<EdgeIndex>> {
    let n = cycle.len();
    let mut potential_fences_paths: Vec<Vec<Vec<_>>> = vec![];
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

        // Check the node types between u and v
        match (aeg.config.architecture, &aeg.graph[u], &aeg.graph[v]) {
            // Only alow (po)WR edges on TSO
            (Architecture::Tso, Node::Write(_, _), Node::Read(_, _)) => {}
            (Architecture::Tso, _, _) => continue,
            (Architecture::Arm, _, _) => unimplemented!(),
            // Allow everything on Power
            (Architecture::Power, _, _) => {}
        }

        let paths: Vec<Vec<_>> = all_simple_po_paths(aeg, u, v, 0, None)
            .map(|path| {
                path.windows(2)
                    .map(|pair| aeg.graph.find_edge(pair[0], pair[1]).unwrap())
                    .collect()
            })
            .collect();
        if !paths.is_empty() {
            potential_fences_paths.push(paths);
        }
    }

    // Each item in potential fences paths contains a list of paths
    // from two nodes in the cycle, hence potential fences paths is
    // a triply nested loop. Here we reduce the loop into all the
    // potential paths that result from it by taking the cartesian product

    // As an example:
    //
    // [
    //   [
    //     [a, z ,b]
    //   ],
    //   [
    //     [y, c],
    //     [x, c]
    //   ]
    // ]
    //
    // turns into
    //
    // [
    //   [a, z, b, y, c],
    //   [a, z, b, x, c]
    // ]
    potential_fences_paths.iter().fold(vec![vec![]], |acc, e| {
        acc.iter()
            .cartesian_product(e)
            .map(|(t1, t2)| t1.iter().chain(t2).copied().collect::<Vec<_>>())
            .collect()
    })
}

#[cfg(test)]
mod test {
    use petgraph::dot::Dot;

    use super::Architecture::*;
    use super::*;
    use crate::aeg::{AbstractEventGraph, Aeg, AegConfig, AegEdge};

    #[test]
    #[allow(non_snake_case)]
    fn minimal_cycle() {
        let mut g = Aeg::new();

        let Wy = g.add_node(Node::Write(0, 0));
        let Rx = g.add_node(Node::Read(0, 1));

        let Wx = g.add_node(Node::Write(1, 1));
        let Ry = g.add_node(Node::Read(1, 0));

        g.update_edge(Wy, Rx, AegEdge::ProgramOrder);
        g.update_edge(Wx, Ry, AegEdge::ProgramOrder);

        g.update_edge(Rx, Wx, AegEdge::Competing);
        g.update_edge(Wx, Rx, AegEdge::Competing);

        g.update_edge(Ry, Wy, AegEdge::Competing);
        g.update_edge(Wy, Ry, AegEdge::Competing);

        let aeg = AbstractEventGraph {
            graph: g,
            config: Default::default(),
            var_map: Default::default(),
            thread_map: Default::default(),
        };

        let mut mc = IncompleteCriticalCycle::with_architecture(Architecture::Tso);
        assert!(mc.add_node(&aeg, Wy));

        let mc2 = IncompleteCriticalCycle::from_cycle(mc.clone().cycle, &aeg);
        assert_eq!(mc2, mc);

        assert!(mc.add_node(&aeg, Rx));

        let mc2 = IncompleteCriticalCycle::from_cycle(mc.clone().cycle, &aeg);
        assert_eq!(mc2, mc);
    }

    #[test]
    #[should_panic(expected = "node added is not a successor of the last in the cycle")]
    #[allow(non_snake_case)]
    fn minimal_cycle_panics() {
        let mut g = Aeg::new();

        let Wy = g.add_node(Node::Write(0, 0));
        let Rx = g.add_node(Node::Read(0, 1));

        let Wx = g.add_node(Node::Write(1, 1));
        let Ry = g.add_node(Node::Read(1, 0));

        g.update_edge(Wy, Rx, AegEdge::ProgramOrder);
        g.update_edge(Wx, Ry, AegEdge::ProgramOrder);

        g.update_edge(Rx, Wx, AegEdge::Competing);
        g.update_edge(Wx, Rx, AegEdge::Competing);

        g.update_edge(Ry, Wy, AegEdge::Competing);
        g.update_edge(Wy, Ry, AegEdge::Competing);

        let aeg = AbstractEventGraph {
            graph: g,
            config: Default::default(),
            var_map: Default::default(),
            thread_map: Default::default(),
        };

        let mut mc = IncompleteCriticalCycle::with_architecture(Architecture::Tso);
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
        let mut aeg = AbstractEventGraph::new(&ast);
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));

        // TSO
        let ccs = critical_cycles(&aeg);
        assert_eq!(ccs.len(), 1);

        // Power
        aeg.config.architecture = Power;
        let ccs = critical_cycles(&aeg);
        assert_eq!(ccs.len(), 1);
    }

    #[test]
    fn simple_critical_cycle_skip() {
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
        let aeg = AbstractEventGraph::with_config(
            &ast,
            AegConfig {
                architecture: Architecture::Power,
            },
        );
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));

        let ccs = critical_cycles(&aeg);
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
        let mut aeg = AbstractEventGraph::with_config(
            &ast,
            AegConfig {
                architecture: Power,
            },
        );
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));

        // Power
        let ccs = critical_cycles(&aeg);
        dbg!(&ccs);

        // There are actually 7 critical cycles, 3 more than the 4 suggested in the fig 16
        assert_eq!(ccs.len(), 7);

        // TSO
        aeg.config.architecture = Architecture::Tso;
        let ccs = critical_cycles(&aeg);
        dbg!(&ccs);
        assert_eq!(ccs.len(), 2);
        assert_eq!(ccs[0].potential_fences.len(), 2)
    }

    #[test]
    fn multiple_ccs_one_source() {
        // In this program the first Wx has 2 critical cycles originating from it
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        thread t1 {
            x = 1;
            let a: u32 = y;
            let b: u32 = y;
        }
        thread t2 {
            y = 1;
            let d: u32 = x;
        }
        final {}
        "#;

        let program = parser::parse(program).unwrap();

        let aeg = AbstractEventGraph::new(&program);
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));
        let ccs = critical_cycles(&aeg);
        dbg!(&ccs);
        assert_eq!(ccs.len(), 2);

        // Now it only has one
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        thread t1 {
            x = 1;
            let a: u32 = y;
            let b: u32 = x;
        }
        thread t2 {
            y = 1;
            let d: u32 = x;
        }
        final {}
        "#;
        let program = parser::parse(program).unwrap();
        let ccs = critical_cycles(&AbstractEventGraph::new(&program));
        assert_eq!(ccs.len(), 1);
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
            y = 1;
        }
        thread t2 {
            let b: u32 = y;
            let c: u32 = z;
            let d: u32 = x;
        }
        final {}
        "#;

        let program = parser::parse(program).unwrap();

        let aeg = AbstractEventGraph::with_config(
            &program,
            AegConfig {
                architecture: Power,
            },
        );
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));
        let ccs = critical_cycles(&aeg);
        dbg!(&ccs);
        // There are 2 critical cycles, but one is duplicated because of the pf's that are different accross each branch
        assert_eq!(ccs.len(), 3);

        // The cycle accross the branch
        assert_eq!(ccs[0].potential_fences.len(), 4);
        assert_eq!(ccs[1].potential_fences.len(), 4);

        // The cycle from inside the branch
        assert_eq!(ccs[2].potential_fences.len(), 3);
    }

    #[test]
    fn nested_ifs() {
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        let z: u32 = 0;
        thread t1 {
            let a: u32 = 0;
            x = 42;
            if (1 == 1) {
                if (1 == 1 ) {
                    z = 1;
                } else {
                    z = 2;
                }
            } else {
                if (1 == 1) {
                    z = 3;
                } else {
                    z = 4; 
                }
            }
            y = 1;
        }
        thread t2 {
            let b: u32 = y;
            let d: u32 = x;
        }
        final {}
        "#;

        let program = parser::parse(program).unwrap();

        let aeg = AbstractEventGraph::with_config(
            &program,
            AegConfig {
                architecture: Power,
            },
        );
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));
        let ccs = critical_cycles(&aeg);
        dbg!(&ccs);
        // 1 critical cycle with 4 branches
        assert_eq!(ccs.len(), 4);
        let cycle = ccs[0].cycle.clone();
        assert!(ccs.into_iter().map(|cc| cc.cycle).all(|c| c == cycle));
    }

    #[test]
    fn cc_through_nested_ifs() {
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        let z: u32 = 0;
        thread t1 {
            let a: u32 = 0;
            if (1 == 1) {
                x = 42;
                if (1 == 1 ) {
                    z = 1;
                } else {
                    z = 2;
                }
            } else {
                if (1 == 1) {
                    z = 3;
                } else {
                    z = 4; 
                }
            }
            y = 1;
        }
        thread t2 {
            let b: u32 = y;
            let d: u32 = x;
        }
        final {}
        "#;

        let program = parser::parse(program).unwrap();

        let aeg = AbstractEventGraph::with_config(
            &program,
            AegConfig {
                architecture: Power,
            },
        );
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));
        let ccs = critical_cycles(&aeg);
        dbg!(&ccs);
        assert_eq!(ccs.len(), 2);
    }

    #[test]
    fn cc_through_middle_of_while() {
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        let z: u32 = 0;
        thread t1 {
            while (x == 0) {
                y = 1;
                x = 1;
            }
            z = 1;
        }
        thread t2 {
            let a: u32 = z;
            let b: u32 = y;
        }
        final {}
        "#;

        let program = parser::parse(program).unwrap();

        let aeg = AbstractEventGraph::with_config(
            &program,
            AegConfig {
                architecture: Power,
            },
        );

        dbg!(Dot::new(&aeg.graph));

        let ccs = critical_cycles(&aeg);
        assert_eq!(ccs.len(), 1);
        dbg!(&ccs);

        let actual = CriticalCycle {
            cycle: vec![
                NodeIndex::from(1),
                NodeIndex::from(3),
                NodeIndex::from(4),
                NodeIndex::from(5),
            ],
            potential_fences: vec![
                EdgeIndex::from(1),
                EdgeIndex::from(2),
                EdgeIndex::from(3),
                EdgeIndex::from(4),
            ],
        };

        assert_eq!(ccs[0].cycle, actual.cycle);
        assert_eq!(ccs[0].potential_fences, actual.potential_fences);
    }

    #[test]
    fn cc_over_a_while() {
        let program = r#"
        let x: u32 = 0;
        let y: u32 = 0;
        let z: u32 = 0;
        thread t1 {
            y = 1;
            while (x == 0) {
                x = 1;
            }
            z = 1;
        }
        thread t2 {
            let a: u32 = z;
            let b: u32 = y;
        }
        final {}
        "#;

        let program = parser::parse(program).unwrap();

        let aeg = AbstractEventGraph::with_config(
            &program,
            AegConfig {
                architecture: Power,
            },
        );

        dbg!(Dot::new(&aeg.graph));

        let ccs = critical_cycles(&aeg);
        dbg!(&ccs);

        assert_eq!(ccs.len(), 1);

        let actual = CriticalCycle {
            cycle: vec![
                NodeIndex::from(0),
                NodeIndex::from(3),
                NodeIndex::from(4),
                NodeIndex::from(5),
            ],
            potential_fences: vec![EdgeIndex::from(0), EdgeIndex::from(3), EdgeIndex::from(4)],
        };

        assert_eq!(ccs[0].cycle, actual.cycle);
        assert_eq!(ccs[0].potential_fences, actual.potential_fences);
    }

    #[test]
    fn ifs_and_whiles_dekker_sc() {
        let program = include_str!("../../programs/dekker-sc.toy");

        let program = parser::parse(program).unwrap();

        let aeg = AbstractEventGraph::with_config(&program, AegConfig { architecture: Tso });

        let ccs = critical_cycles(&aeg);
        // I'm not actually sure this is the correct amount, but it's to check for regressions
        assert_eq!(ccs.len(), 19);
    }

    #[test]
    fn lamport_3() {
        let program = include_str!("../../programs/lamport-3.toy");

        let program = parser::parse(program).unwrap();

        let aeg = AbstractEventGraph::with_config(&program, AegConfig { architecture: Tso });

        let ccs = critical_cycles(&aeg);
        // I'm not actually sure this is the correct amount, but it's to check for regressions
        assert_eq!(ccs.len(), 5161);
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
        let program = parser::parse(program).unwrap();
        let aeg = AbstractEventGraph::with_config(
            &program,
            AegConfig {
                architecture: Power,
            },
        );
        println!("{:?}", Dot::with_config(&aeg.graph, &[]));

        let ccs = critical_cycles(&aeg);
        assert_eq!(ccs.len(), 0);
    }
}
