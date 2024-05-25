use petgraph::visit::{GraphBase, GraphRef, VisitMap, Visitable};

use crate::aeg::{Aeg, AegEdge};

/// A modified version of [petgraph::visit::ProgramOrderDfs],
#[derive(Clone, Debug)]
pub struct ProgramOrderDfs<VM> {
    /// The stack of nodes to visit
    pub stack: Vec<<Aeg as GraphBase>::NodeId>,
    /// The map of discovered nodes
    pub discovered: VM,
}

impl<VM> Default for ProgramOrderDfs<VM>
where
    VM: Default,
{
    fn default() -> Self {
        ProgramOrderDfs {
            stack: Vec::new(),
            discovered: VM::default(),
        }
    }
}

impl<VM> ProgramOrderDfs<VM>
where
    VM: VisitMap<<Aeg as GraphBase>::NodeId>,
{
    /// Create a new **ProgramOrderDfs**, using the graph's visitor map, and put **start**
    /// in the stack of nodes to visit.
    pub fn new<G>(graph: G, start: <Aeg as GraphBase>::NodeId) -> Self
    where
        G: GraphRef + Visitable<NodeId = <Aeg as GraphBase>::NodeId, Map = VM>,
    {
        let mut dfs = ProgramOrderDfs::empty(graph);
        dfs.move_to(start);
        dfs
    }

    /// Create a `ProgramOrderDfs` from a vector and a visit map
    pub fn from_parts(stack: Vec<<Aeg as GraphBase>::NodeId>, discovered: VM) -> Self {
        ProgramOrderDfs { stack, discovered }
    }

    /// Clear the visit state
    pub fn reset<G>(&mut self, graph: G)
    where
        G: GraphRef + Visitable<NodeId = <Aeg as GraphBase>::NodeId, Map = VM>,
    {
        graph.reset_map(&mut self.discovered);
        self.stack.clear();
    }

    /// Create a new **ProgramOrderDfs** using the graph's visitor map, and no stack.
    pub fn empty<G>(graph: G) -> Self
    where
        G: GraphRef + Visitable<NodeId = <Aeg as GraphBase>::NodeId, Map = VM>,
    {
        ProgramOrderDfs {
            stack: Vec::new(),
            discovered: graph.visit_map(),
        }
    }

    /// Keep the discovered map, but clear the visit stack and restart
    /// the dfs from a particular node.
    pub fn move_to(&mut self, start: <Aeg as GraphBase>::NodeId) {
        self.stack.clear();
        self.stack.push(start);
    }

    /// Return the next node in the dfs, or **None** if the traversal is done.
    pub fn next(&mut self, graph: &Aeg) -> Option<<Aeg as GraphBase>::NodeId> {
        while let Some(node) = self.stack.pop() {
            if self.discovered.visit(node) {
                for succ in graph.neighbors(node) {
                    if !self.discovered.is_visited(&succ)
                        && matches!(
                            graph[graph.find_edge(node, succ).unwrap()],
                            AegEdge::ProgramOrder
                        )
                    {
                        self.stack.push(succ);
                    }
                }
                return Some(node);
            }
        }
        None
    }
}
