struct Link{
    bandwidth_gbps: f64,
    latency_ns: u64,
    buffer_size: usize
}

struct TopologyGraph{
    graph:petgraph::graph::UnGraph<u64,()>
}