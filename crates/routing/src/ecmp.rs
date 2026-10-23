//! Equal-Cost Multi-Path (ECMP) routing.
//! Uses a hash of (src, dst, flow_id) to pick among shortest paths.

use std::collections::{HashMap, VecDeque};
use petgraph::graph::{EdgeIndex, NodeIndex};
use sim_core::Router;
use topology::TopologyGraph;

pub struct EcmpRouter;

impl Router for EcmpRouter {
    fn route(
        &self,
        graph: &TopologyGraph,
        src: NodeIndex,
        dst: NodeIndex,
        flow_id: u64,
    ) -> Option<Vec<EdgeIndex>> {
        if src == dst {
            return Some(vec![]);
        }

        // Simple FNV-1a hash for deterministic path selection
        let mut hash = 14695981039346656037u64;
        let mix = |hash: &mut u64, val: u64| {
            *hash ^= val;
            *hash = hash.wrapping_mul(1099511628211);
        };
        mix(&mut hash, src.index() as u64);
        mix(&mut hash, dst.index() as u64);
        mix(&mut hash, flow_id);

        // BFS that groups nodes by distance and picks one parent per node.
        // To achieve ECMP, we find ALL shortest paths and select among
        // parents based on the hash.
        let mut dist: HashMap<NodeIndex, usize> = HashMap::new();
        let mut parents: HashMap<NodeIndex, Vec<(NodeIndex, EdgeIndex)>> = HashMap::new();
        let mut q = VecDeque::new();

        dist.insert(src, 0);
        q.push_back(src);

        while let Some(u) = q.pop_front() {
            let d = dist[&u];
            for (v, _) in graph.neighbors_with_links(u) {
                let eid = graph.inner().find_edge(u, v).unwrap();
                if let Some(&vd) = dist.get(&v) {
                    if vd == d + 1 {
                        parents.entry(v).or_default().push((u, eid));
                    }
                } else {
                    dist.insert(v, d + 1);
                    parents.insert(v, vec![(u, eid)]);
                    q.push_back(v);
                }
            }
        }

        if !dist.contains_key(&dst) {
            return None;
        }

        // Reconstruct path backwards, picking a parent based on hash
        let mut path = Vec::new();
        let mut cur = dst;
        while cur != src {
            let candidates = &parents[&cur];
            // Use hash modulo to pick parent
            let idx = (hash as usize) % candidates.len();
            let (parent, eid) = candidates[idx];
            path.push(eid);
            cur = parent;

            // LCG step: mix the hash thoroughly for the next hop
            // (constants from Numerical Recipes)
            hash = hash
                .wrapping_mul(1664525)
                .wrapping_add(1013904223);
        }
        path.reverse();
        Some(path)
    }
}