use petgraph::graph::NodeIndex;
use sim_core::Flow;

/// Generates flows for a Tree All-Reduce operation.
/// Simple binomial tree: reduce to root (GPU 0), then broadcast.
pub fn generate(num_gpus: usize, data_size: u64) -> Vec<Flow> {
    if num_gpus <= 1 { return vec![]; }

    let mut flows = Vec::new();
    let mut flow_id = 0;
    let chunk_size = data_size / (num_gpus as u64);

    // Phase 1: Reduce (leaves to root)
    let mut step = 1;
    while step < num_gpus {
        for i in (0..num_gpus).step_by(step * 2) {
            if i + step < num_gpus {
                flows.push(Flow {
                    id: flow_id,
                    src: NodeIndex::new(i + step),
                    dst: NodeIndex::new(i),
                    size_bytes: chunk_size,
                    start_time_ns: 0,
                });
                flow_id += 1;
            }
        }
        step *= 2;
    }

    // Phase 2: Broadcast (root to leaves)
    step = num_gpus / 2;
    while step > 0 {
        for i in (0..num_gpus).step_by(step * 2) {
            if i + step < num_gpus {
                flows.push(Flow {
                    id: flow_id,
                    src: NodeIndex::new(i),
                    dst: NodeIndex::new(i + step),
                    size_bytes: chunk_size,
                    start_time_ns: 0,
                });
                flow_id += 1;
            }
        }
        step /= 2;
    }

    flows
}