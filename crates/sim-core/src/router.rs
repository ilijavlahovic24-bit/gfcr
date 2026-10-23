use std::collections::HashMap;
use petgraph::graph::{EdgeIndex, NodeIndex};
use topology::TopologyGraph;

/// Trait for routing engines. `sim-core` does not depend on concrete
/// routing implementations (ECMP, adaptive), it only knows this trait.
pub trait Router {
    fn route(
        &self,
        graph: &TopologyGraph,
        src: NodeIndex,
        dst: NodeIndex,
        flow_id: u64,
        link_load: &HashMap<EdgeIndex, f64>,
    ) -> Option<Vec<EdgeIndex>>;
}