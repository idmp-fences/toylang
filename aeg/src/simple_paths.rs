use std::iter::{from_fn, FromIterator};

use indexmap::IndexSet;

use crate::{aeg::Aeg, AbstractEventGraph};

use petgraph::visit::GraphBase;

/// Returns an iterator that produces all simple paths from `from` node to `to` through program order edges, which contains at least `min_intermediate_nodes` nodes
/// and at most `max_intermediate_nodes`, if given, or limited by the (po sub)graph's order otherwise. The simple path is a path without repetitions.
///
/// This algorithm is adapted from [`petgraph::algo::all_simple_paths`].
pub fn all_simple_po_paths(
    aeg: &AbstractEventGraph,
    from: <Aeg as GraphBase>::NodeId,
    to: <Aeg as GraphBase>::NodeId,
    min_intermediate_nodes: usize,
    max_intermediate_nodes: Option<usize>,
) -> impl Iterator<Item = Vec<<Aeg as GraphBase>::NodeId>> + '_ {
    // how many nodes are allowed in simple path up to target node
    // it is min/max allowed path length minus one, because it is more appropriate when implementing lookahead
    // than constantly add 1 to length of current path
    let max_length = if let Some(l) = max_intermediate_nodes {
        l + 1
    } else {
        aeg.graph.node_count() - 1
    };

    let min_length = min_intermediate_nodes + 1;

    // list of visited nodes
    let mut visited: IndexSet<<Aeg as GraphBase>::NodeId> = IndexSet::from_iter(Some(from));
    // list of childs of currently exploring path nodes,
    // last elem is list of childs of last visited node
    let mut stack = vec![aeg.close_po_neighbors(from)];

    from_fn(move || {
        while let Some(children) = stack.last_mut() {
            if let Some(child) = children.next() {
                if visited.len() < max_length {
                    if child == to {
                        if visited.len() >= min_length {
                            let path = visited.iter().cloned().chain(Some(to)).collect();
                            return Some(path);
                        }
                    } else if !visited.contains(&child) {
                        visited.insert(child);
                        stack.push(aeg.close_po_neighbors(child));
                    }
                } else {
                    if (child == to || children.any(|v| v == to)) && visited.len() >= min_length {
                        let path = visited.iter().cloned().chain(Some(to)).collect();
                        return Some(path);
                    }
                    stack.pop();
                    visited.pop();
                }
            } else {
                stack.pop();
                visited.pop();
            }
        }
        None
    })
}

#[cfg(test)]
mod test {

    use itertools::Itertools;

    use crate::AbstractEventGraph;

    use super::all_simple_po_paths;

    #[test]
    fn cartesian_product() {
        let paths = vec![
            vec![vec![0, 5, 1]],
            vec![vec![2, 6, 3], vec![2, 7, 8, 3]],
            vec![vec![4, 6], vec![4, 5, 6]],
        ];

        let p = paths.iter().fold(vec![vec![]], |acc, e| {
            acc.iter()
                .cartesian_product(e)
                .map(|(t1, t2)| {
                    t1.into_iter()
                        .chain(t2.into_iter())
                        .copied()
                        .collect::<Vec<_>>()
                })
                .collect()
        });
        dbg!(p);
    }

    #[test]
    fn ifs() {
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
        final {}
        "#;

        let program = parser::parse(program).unwrap();

        let aeg = dbg!(AbstractEventGraph::new(&program));

        let first_wx = aeg.graph.node_indices().next().unwrap();
        let last_wx = aeg.graph.node_indices().next_back().unwrap();

        let paths = all_simple_po_paths(&aeg, first_wx, last_wx, 0, None);

        // Three possible paths: one per branch
        assert_eq!(dbg!(paths.collect::<Vec<_>>()).len(), 2);
    }

    #[test]
    fn whiles() {
        let program = r#"
        let x: u32 = 0;
        thread t1 {
            while (x == 0) {
                x = 1;
            }
            x = 2;
        }
        final {}
        "#;

        let program = parser::parse(program).unwrap();

        let aeg = dbg!(AbstractEventGraph::new(&program));

        let first_wx = aeg.graph.node_indices().next().unwrap();
        let last_wx = aeg.graph.node_indices().next_back().unwrap();

        let paths = all_simple_po_paths(&aeg, first_wx, last_wx, 0, None);

        // Either we skip the while loop or we take it
        assert_eq!(dbg!(paths.collect::<Vec<_>>()).len(), 2);
    }
}
