use petgraph::graph::NodeIndex;
use sim_core::Flow;

/// Generates flows for Pipeline parallelism.
/// GPU `i` sends a microbatch to GPU `i+1`.
pub fn generate(num_stages: usize, microbatch_size: u64) -> Vec<Flow> {
    let mut flows = Vec::with_capacity(num_stages.saturating_sub(1));
    let mut flow_id = 0;

    for i in 0..num_stages.saturating_sub(1) {
        flows.push(Flow {
            id: flow_id,
            src: NodeIndex::new(i),
            dst: NodeIndex::new(i + 1),
            size_bytes: microbatch_size,
            start_time_ns: 0,
        });
        flow_id += 1;
    }

    flows
}