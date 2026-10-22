use petgraph::graph::NodeIndex;
use sim_core::Flow;

/// Generates flows for a Ring All-Reduce operation.
/// The total `data_size` is divided into `num_gpus` chunks.
/// Each GPU participates in 2*(N-1) chunk transfers (N-1 for reduce-scatter,
/// N-1 for all-gather).
pub fn generate(num_gpus: usize, data_size: u64) -> Vec<Flow> {
    if num_gpus <= 1 {
        return vec![];
    }

    let chunk_size = data_size / (num_gpus as u64);
    let mut flows = Vec::with_capacity(2 * num_gpus * (num_gpus - 1));
    let mut flow_id = 0;

    // Steps 0..(N-1): Reduce-Scatter phase
    // Steps (N-1)..2(N-1): All-Gather phase
    // In both phases, GPU `i` sends to GPU `(i+1) % N`.
    for _step in 0..(2 * (num_gpus - 1)) {
        for i in 0..num_gpus {
            flows.push(Flow {
                id: flow_id,
                src: NodeIndex::new(i),
                dst: NodeIndex::new((i + 1) % num_gpus),
                size_bytes: chunk_size,
                start_time_ns: 0, // Can be staggered later by the caller
            });
            flow_id += 1;
        }
    }

    flows
}