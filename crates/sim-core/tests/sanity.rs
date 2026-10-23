//! Faza 2 sanity checks: single-flow FCT must match the analytical minimum
//! within ±5%. Also checks two disjoint flows don't interfere.

use std::collections::HashMap;
use petgraph::graph::{EdgeIndex, NodeIndex};
use sim_core::{Flow, Router, Simulator};
use sim_core::path::shortest_path;
use topology::fat_tree::generate;
use topology::graph::Link;
struct MockRouter;
impl Router for MockRouter {
    fn route(
        &self,
        graph: &topology::TopologyGraph,
        src: NodeIndex,
        dst: NodeIndex,
        _flow_id: u64,
        _link_load: &HashMap<EdgeIndex, f64>, 
    ) -> Option<Vec<EdgeIndex>> {
        shortest_path(graph, src, dst)
    }
}
fn k4_graph() -> topology::TopologyGraph {
    generate(
        4,
        Link {
            bandwidth_gbps: 100.0,
            latency_ns: 500,
            buffer_size: 100_000,
        },
    )
        .unwrap()
}

fn host_at(g: &topology::TopologyGraph, gpu_id: usize) -> NodeIndex {
    g.hosts()
        .into_iter()
        .find(|id| matches!(
            g.node(*id).unwrap().kind,
            topology::NodeKind::Host { gpu_id: id_g } if id_g == gpu_id
        ))
        .unwrap()
}

#[test]
fn single_flow_across_pods_matches_theory() {
    // Hosts 0 and 15 are in different pods → 6-hop path.
    let g = k4_graph();
    let src = host_at(&g, 0);
    let dst = host_at(&g, 15);

    let flow = Flow {
        id: 1,
        src,
        dst,
        size_bytes: 1024,
        start_time_ns: 0,
    };

    let result = Simulator::new(g, vec![flow], Box::new(MockRouter)).run();

    assert_eq!(result.completed_flows.len(), 1);
    let fct = result.completed_flows[0].fct_ns;

    // Theory: 6 hops × 500 ns prop + 1024 B at 12.5 bytes/ns
    let path_latency = 6 * 500; // 3000
    let tx_time = (1024.0_f64 / 12.5).ceil() as u64; // 82
    let theoretical = path_latency + tx_time;

    let tol = (theoretical as f64 * 0.05) as u64;
    let lo = theoretical - tol;
    let hi = theoretical + tol;

    assert!(
        fct >= lo && fct <= hi,
        "FCT {fct} ns outside [{lo}, {hi}] (theoretical {theoretical} ns)"
    );
}

#[test]
fn single_flow_same_pod_matches_theory() {
    // Hosts 0 and 2 are in the same pod, different edge switches → 4-hop path.
    let g = k4_graph();
    let src = host_at(&g, 0);
    let dst = host_at(&g, 2);

    let flow = Flow {
        id: 1,
        src,
        dst,
        size_bytes: 1024,
        start_time_ns: 0,
    };

    let result = Simulator::new(g, vec![flow], Box::new(MockRouter)).run();
    let fct = result.completed_flows[0].fct_ns;

    let path_latency = 4 * 500; // 2000
    let tx_time = (1024.0_f64 / 12.5).ceil() as u64; // 82
    let theoretical = path_latency + tx_time;

    let tol = (theoretical as f64 * 0.05) as u64;
    assert!(
        fct >= theoretical - tol && fct <= theoretical + tol,
        "FCT {fct} outside ±5% of {theoretical}"
    );
}

#[test]
fn two_disjoint_flows_do_not_interfere() {
    // Hosts 0→1 (same edge switch) and hosts 4→5 (same edge switch, pod 1).
    // Disjoint paths: each should get full bandwidth.
    let g = k4_graph();
    let flows = vec![
        Flow {
            id: 1,
            src: host_at(&g, 0),
            dst: host_at(&g, 1),
            size_bytes: 1024,
            start_time_ns: 0,
        },
        Flow {
            id: 2,
            src: host_at(&g, 4),
            dst: host_at(&g, 5),
            size_bytes: 1024,
            start_time_ns: 0,
        },
    ];

    // ISPRAVLJENO: vec![flow] -> flows
    let result = Simulator::new(g, flows, Box::new(MockRouter)).run();
    assert_eq!(result.completed_flows.len(), 2);

    // Both are 2-hop paths (host–edge–host), so ~1000 + 82 ns expected.
    // Because they're disjoint, both should be near the single-flow FCT.
    for f in &result.completed_flows {
        assert!(
            f.fct_ns < 1200,
            "flow {} FCT {} ns too large — flows interfered?",
            f.id,
            f.fct_ns
        );
    }
}