use routing::ecmp::EcmpRouter;
use sim_core::Simulator;
use topology::fat_tree;
use topology::graph::Link;
use traffic_gen::{all_to_all, ring_allreduce};
#[test]
fn ring_all_reduce_matches_theory() {
    let n = 4;
    let data_size = 1024 * 1024; // 1 MB
    let bandwidth_gbps = 100.0;
    let bandwidth_bpns = bandwidth_gbps * 0.125;
    let latency_ns = 10;

    let chunk_size = data_size / n;
    let tx_time_ns = (chunk_size as f64 / bandwidth_bpns).ceil() as u64;

    // A path between different pods in k=4 fat-tree is exactly 6 hops.
    let step_duration_ns = tx_time_ns + 6 * latency_ns;

    let mut flows = ring_allreduce::generate(n, data_size as u64);

    // Stagger start times to simulate the sequential logical steps
    for flow in &mut flows {
        let step = flow.id / (n as u64);
        flow.start_time_ns = step * step_duration_ns;
    }

    let g = fat_tree::generate(4, Link {
        bandwidth_gbps,
        latency_ns,
        buffer_size: 100_000,
    }).unwrap();

    // Map to hosts in different pods (0, 4, 8, 12) for constant 6-hop path
    let hosts = g.hosts();
    let target_hosts = vec![hosts[0], hosts[4], hosts[8], hosts[12]];

    for flow in &mut flows {
        flow.src = target_hosts[flow.src.index()];
        flow.dst = target_hosts[flow.dst.index()];
    }

    let result = Simulator::new(g, flows, Box::new(EcmpRouter)).run();

    // Theoretical transmission time: 2*(N-1)/N * data_size / bandwidth
    let theoretical_tx = 2.0 * (n - 1) as f64 / n as f64 * (data_size as f64 / bandwidth_bpns);

    // Last flow also finishes its 6-hop propagation
    let expected_fct = theoretical_tx + (6 * latency_ns) as f64;
    
    // The total completion time is when the last flow finishes.
    let max_end_time = result.completed_flows.iter().map(|f| f.end_time_ns).max().unwrap() as f64;

    let diff = (max_end_time - expected_fct).abs();
    assert!(diff / expected_fct < 0.05, "Total time {} outside 5% of expected {}", max_end_time, expected_fct);
}

#[test]
fn ring_all_reduce_flow_count_and_size() {
    let n = 8;
    let data_size = 1024 * 1024;
    let flows = ring_allreduce::generate(n, data_size);

    // N GPUs * 2*(N-1) steps
    assert_eq!(flows.len(), n * 2 * (n - 1));

    let total_bytes: u64 = flows.iter().map(|f| f.size_bytes).sum();
    assert_eq!(total_bytes, (n * 2 * (n - 1)) as u64 * (data_size / n as u64));
}

#[test]
fn all_to_all_flow_count() {
    let n = 16;
    let size = 1024;
    let flows = all_to_all::generate(n, size);
    assert_eq!(flows.len(), n * (n - 1));
    assert!(flows.iter().all(|f| f.size_bytes == size));
}