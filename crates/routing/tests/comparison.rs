use routing::{adaptive::AdaptiveRouter, ecmp::EcmpRouter};
use sim_core::{Flow, Simulator};
use topology::{fat_tree, graph::Link};

fn k4_graph() -> topology::TopologyGraph {
    fat_tree::generate(4, Link {
        bandwidth_gbps: 100.0,
        latency_ns: 500,
        buffer_size: 100_000,
    }).unwrap()
}

#[test]
fn adaptive_router_beats_ecmp_on_tail_latency() {
    let g = k4_graph();
    let hosts = g.hosts();

    // Generate heavy all-to-all traffic that will congest the network
    let mut flows = vec![];
    let mut id = 0;
    for i in 0..4 {
        for j in 0..4 {
            if i == j { continue; }
            flows.push(Flow {
                id,
                src: hosts[i],
                dst: hosts[j],
                size_bytes: 2 * 1024 * 1024, // 2 MB
                start_time_ns: 0,
            });
            id += 1;
        }
    }

    // Run ECMP
    let res_ecmp = Simulator::new(k4_graph(), flows.clone(), Box::new(EcmpRouter)).run();
    let ecmp_p99 = {
        let mut fcts: Vec<u64> = res_ecmp.completed_flows.iter().map(|f| f.fct_ns).collect();
        fcts.sort_unstable();
        fcts[(fcts.len() as f64 * 0.99) as usize]
    };

    // Run Adaptive
    let res_adaptive = Simulator::new(k4_graph(), flows, Box::new(AdaptiveRouter)).run();
    let adaptive_p99 = {
        let mut fcts: Vec<u64> = res_adaptive.completed_flows.iter().map(|f| f.fct_ns).collect();
        fcts.sort_unstable();
        fcts[(fcts.len() as f64 * 0.99) as usize]
    };

    println!("ECMP p99 FCT:     {} ns", ecmp_p99);
    println!("Adaptive p99 FCT: {} ns", adaptive_p99);

    assert!(adaptive_p99 < ecmp_p99,
            "Adaptive p99 ({}) should be lower than ECMP p99 ({})",
            adaptive_p99, ecmp_p99);
}