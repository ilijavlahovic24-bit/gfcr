//! Path finding.
//!
//! MVP: BFS shortest path by hop count. On fat-tree with uniform link
//! latencies this is also shortest by latency. Replaced by the `routing`
//! crate in Faza 4 (ECMP) and Faza 5 (adaptive).

use std::collections::{HashMap, VecDeque};

use petgraph::graph::{EdgeIndex, NodeIndex};
use topology::TopologyGraph;

/// BFS shortest path from `src` to `dst`. Returns the sequence of edge
/// indices traversed, or `None` if `dst` is not reachable from `src`.
pub fn shortest_path(
    graph: &TopologyGraph,
    src: NodeIndex,
    dst: NodeIndex,
) -> Option<Vec<EdgeIndex>> {
    if src == dst {
        return Some(vec![]);
    }

    // parent[v] = (predecessor of v, edge from predecessor to v)
    let mut parent: HashMap<NodeIndex, (NodeIndex, EdgeIndex)> = HashMap::new();
    let mut visited: HashMap<NodeIndex, ()> = HashMap::new();
    visited.insert(src, ());

    let mut q = VecDeque::new();
    q.push_back(src);

    while let Some(u) = q.pop_front() {
        for (v, _) in graph.neighbors_with_links(u) {
            if visited.insert(v, ()).is_none() {
                let eid = graph
                    .inner()
                    .find_edge(u, v)
                    .expect("neighbors_with_links returned an edge");
                parent.insert(v, (u, eid));
                if v == dst {
                    return Some(reconstruct(&parent, src, dst));
                }
                q.push_back(v);
            }
        }
    }

    None
}

fn reconstruct(
    parent: &HashMap<NodeIndex, (NodeIndex, EdgeIndex)>,
    src: NodeIndex,
    dst: NodeIndex,
) -> Vec<EdgeIndex> {
    let mut path = vec![];
    let mut cur = dst;
    while cur != src {
        let (prev, eid) = parent[&cur];
        path.push(eid);
        cur = prev;
    }
    path.reverse();
    path
}