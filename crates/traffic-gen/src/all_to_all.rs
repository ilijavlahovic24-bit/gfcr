use petgraph::graph::NodeIndex;
use sim_core::Flow;

/// Generates flows for an All-to-All operation (e.g., MoE dispatch/combine).
/// Each GPU sends `data_size_per_pair` bytes to every other GPU.
pub fn generate(num_gpus: usize, data_size_per_pair: u64) -> Vec<Flow> {
    let mut flows = Vec::with_capacity(num_gpus * (num_gpus - 1));
    let mut flow_id = 0;

    for i in 0..num_gpus {
        for j in 0..num_gpus {
            if i == j { continue; }
            flows.push(Flow {
                id: flow_id,
                src: NodeIndex::new(i),
                dst: NodeIndex::new(j),
                size_bytes: data_size_per_pair,
                start_time_ns: 0,
            });
            flow_id += 1;
        }
    }

    flows
}