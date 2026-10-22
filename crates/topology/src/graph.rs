use petgraph::graph::{Graph, NodeIndex};
use petgraph::prelude::EdgeRef;
use petgraph::Undirected;

pub type NodeId = NodeIndex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Host { gpu_id: usize },
    EdgeSwitch { pod: usize, index: usize },
    AggSwitch  { pod: usize, index: usize },
    CoreSwitch { index: usize },
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
}

/// Physical link. For MVP: one undirected edge = one full-duplex link.
/// (the flow-level simulator will treat both directions as independent flows).
#[derive(Debug, Clone)]
pub struct Link {
    pub bandwidth_gbps: f64,
    pub latency_ns: u64,
    pub buffer_size: usize,
}

/// Wrapper around petgraph-a.
/// everything the simulator needs goes through this API, so if we replace the petgraph later, we won't touch all the crates.
pub struct TopologyGraph {
    pub(crate) graph: Graph<Node, Link, Undirected>,
}

impl TopologyGraph {
    pub fn new() -> Self {
        Self { graph: Graph::new_undirected() }
    }

    pub fn node_count(&self) -> usize { self.graph.node_count() }
    pub fn edge_count(&self) -> usize { self.graph.edge_count() }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.graph.node_weight(id)
    }

    pub fn edge_between(&self, a: NodeId, b: NodeId) -> Option<&Link> {
        self.graph.find_edge(a, b).and_then(|e| self.graph.edge_weight(e))
    }

    /// Neighbors + link to each. This will be directly used by the adaptive router.
    pub fn neighbors_with_links(&self, id: NodeId) -> Vec<(NodeId, &Link)> {
        self.graph
            .edges(id)
            .map(|e| {
                let other = if e.source() == id { e.target() } else { e.source() };
                (other, e.weight())
            })
            .collect()
    }

    /// All hosts (GPUs). The traffic generator will use this.
    pub fn hosts(&self) -> Vec<NodeId> {
        self.graph
            .node_indices()
            .filter(|&i| matches!(self.graph[i].kind, NodeKind::Host { .. }))
            .collect()
    }

    /// Connectivity check: BFS from the first node, all must be reachable.
    pub fn is_connected(&self) -> bool {
        if self.graph.node_count() == 0 {
            return true;
        }
        let start = self.graph.node_indices().next().unwrap();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![start];
        visited.insert(start);
        while let Some(n) = stack.pop() {
            for nbr in self.graph.neighbors(n) {
                if visited.insert(nbr) {
                    stack.push(nbr);
                }
            }
        }
        visited.len() == self.graph.node_count()
    }

    pub fn inner(&self) -> &Graph<Node, Link, Undirected> { &self.graph }
    pub(crate) fn inner_mut(&mut self) -> &mut Graph<Node, Link, Undirected> {
        &mut self.graph
    }
}

impl Default for TopologyGraph {
    fn default() -> Self { Self::new() }
}