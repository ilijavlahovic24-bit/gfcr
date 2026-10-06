use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
enum NodeTier {
    Core,
    Aggregation,
    Edge,
    Host,   // GPU/endpoint, not switch
}

pub type NodeId = u64;
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Node {
    id: NodeId,
    tier: NodeTier,
    pod_id: Option<usize>,
}

