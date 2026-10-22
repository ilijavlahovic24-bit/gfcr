use crate::graph::{Link, Node, NodeKind, TopologyGraph};
use crate::config::TopologyError;
/// Generates k-arry fat-treee. k mus be even
pub fn generate(k: usize, link: Link) -> Result<TopologyGraph, TopologyError> {
    if k < 2 || k % 2 != 0 {
        return Err(TopologyError::InvalidK(k));
    }

    let mut g = TopologyGraph::new();
    let half_k = k / 2;

    // --- Core switches: k^2/4, indexed [group, position-in-group]
    // Global index: group*half_k + position
    let n_core = k * k / 4;
    let mut core: Vec<petgraph::graph::NodeIndex> = Vec::with_capacity(n_core);
    for i in 0..n_core {
        core.push(g.inner_mut().add_node(Node {
            kind: NodeKind::CoreSwitch { index: i },
        }));
    }

    // --- Aggregation switches: k^2/2, [sub][position]
    let mut agg: Vec<Vec<petgraph::graph::NodeIndex>> = Vec::with_capacity(k);
    for pod in 0..k {
        let mut row = Vec::with_capacity(half_k);
        for idx in 0..half_k {
            row.push(g.inner_mut().add_node(Node {
                kind: NodeKind::AggSwitch { pod, index: idx },
            }));
        }
        agg.push(row);
    }

    // --- Edge switches: k^2/2, [pod][pos]
    let mut edge: Vec<Vec<petgraph::graph::NodeIndex>> = Vec::with_capacity(k);
    for pod in 0..k {
        let mut row = Vec::with_capacity(half_k);
        for idx in 0..half_k {
            row.push(g.inner_mut().add_node(Node {
                kind: NodeKind::EdgeSwitch { pod, index: idx },
            }));
        }
        edge.push(row);
    }

    // --- Hosts: k^3/4, [pod][edge-pos][host-pos]
    let mut gpu_counter = 0usize;
    let mut hosts: Vec<Vec<Vec<petgraph::graph::NodeIndex>>> = Vec::with_capacity(k);
    for pod in 0..k {
        let mut pod_row: Vec<Vec<petgraph::graph::NodeIndex>> = Vec::with_capacity(half_k);
        for e in 0..half_k {
            let mut row = Vec::with_capacity(half_k);
            for _h in 0..half_k {
                row.push(g.inner_mut().add_node(Node {
                    kind: NodeKind::Host { gpu_id: gpu_counter },
                }));
                gpu_counter += 1;
            }
            pod_row.push(row);
        }
        hosts.push(pod_row);
    }

    // --- Links: host <-> edge
    for pod in 0..k {
        for e in 0..half_k {
            for h in 0..half_k {
                g.inner_mut().add_edge(hosts[pod][e][h], edge[pod][e], link.clone());
            }
        }
    }

    // --- Links: edge <-> agg (full bipartite in pod)
    for pod in 0..k {
        for e in 0..half_k {
            for a in 0..half_k {
                g.inner_mut().add_edge(edge[pod][e], agg[pod][a], link.clone());
            }
        }
    }

    // --- Links: agg <-> core.
    // agg(pod, a) goes to all core switches in group "a":
    // core index a*half_k + q for q in 0..half_k.
    for pod in 0..k {
        for a in 0..half_k {
            for q in 0..half_k {
                let core_idx = a * half_k + q;
                g.inner_mut().add_edge(agg[pod][a], core[core_idx], link.clone());
            }
        }
    }

    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link() -> Link {
        Link { bandwidth_gbps: 100.0, latency_ns: 500, buffer_size: 100_000 }
    }

    #[test]
    fn counts_match_formulas_k4() {
        let g = generate(4, link()).unwrap();
        assert_eq!(g.node_count(), (5 * 4 * 4) / 4); // 20
        assert_eq!(g.edge_count(), (3 * 4 * 4 * 4) / 4); // 48
        assert_eq!(g.hosts().len(), 4 * 4 * 4 / 4); // 16
    }

    #[test]
    fn counts_match_formulas_k8() {
        let g = generate(8, link()).unwrap();
        assert_eq!(g.node_count(), 5 * 8 * 8 / 4); // 80
        assert_eq!(g.edge_count(), 3 * 8 * 8 * 8 / 4); // 384
        assert_eq!(g.hosts().len(), 8 * 8 * 8 / 4); // 128
    }

    #[test]
    fn graph_is_connected() {
        for k in [2, 4, 6, 8] {
            let g = generate(k, link()).unwrap();
            assert!(g.is_connected(), "k={k} not empty");
        }
    }

    #[test]
    fn invalid_k_rejected() {
        assert!(generate(0, link()).is_err());
        assert!(generate(3, link()).is_err()); // odd
    }
}