//! Adaptive Congestion-Aware Router.
//! Uses Dijkstra's algorithm where edge weights are adjusted dynamically
//! based on current link utilization (simulating queueing delay).

use std::collections::{HashMap, BinaryHeap};
use std::cmp::Ordering;
use petgraph::graph::{EdgeIndex, NodeIndex};
use sim_core::Router;
use topology::TopologyGraph;

pub struct AdaptiveRouter;

#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: u64,
    node: NodeIndex,
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap
        other.cost.cmp(&self.cost)
            .then_with(|| self.node.index().cmp(&other.node.index()))
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Router for AdaptiveRouter {
    fn route(
        &self,
        graph: &TopologyGraph,
        src: NodeIndex,
        dst: NodeIndex,
        _flow_id: u64,
        link_load: &HashMap<EdgeIndex, f64>,
    ) -> Option<Vec<EdgeIndex>> {
        if src == dst {
            return Some(vec![]);
        }

        let mut dist: HashMap<NodeIndex, u64> = HashMap::new();
        let mut parent: HashMap<NodeIndex, (NodeIndex, EdgeIndex)> = HashMap::new();
        let mut heap = BinaryHeap::new();

        dist.insert(src, 0);
        heap.push(State { cost: 0, node: src });

        while let Some(State { cost, node }) = heap.pop() {
            if node == dst {
                // Reconstruct path
                let mut path = Vec::new();
                let mut cur = dst;
                while cur != src {
                    let (prev, eid) = parent[&cur];
                    path.push(eid);
                    cur = prev;
                }
                path.reverse();
                return Some(path);
            }

            // If we found a better path to this node already, skip
            if cost > *dist.get(&node).unwrap_or(&u64::MAX) {
                continue;
            }

            for (next, link) in graph.neighbors_with_links(node) {
                let eid = graph.inner().find_edge(node, next).unwrap();

                // Get current load (default 0.0 if not present)
                let load = *link_load.get(&eid).unwrap_or(&0.0);

                // Cost function: Base latency penalized by congestion.
                // We use a cubic function (load^3) to strongly avoid heavily congested links.
                let congestion_penalty = 1.0 + 10.0 * load.powf(3.0);
                let edge_cost = (link.latency_ns as f64 * congestion_penalty) as u64;

                // Ensure minimum cost of 1 to prevent zero-cost loops
                let edge_cost = edge_cost.max(1);

                let next_cost = cost + edge_cost;

                if next_cost < *dist.get(&next).unwrap_or(&u64::MAX) {
                    dist.insert(next, next_cost);
                    parent.insert(next, (node, eid));
                    heap.push(State { cost: next_cost, node: next });
                }
            }
        }

        None // No path found
    }
}