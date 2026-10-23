use petgraph::graph::{EdgeIndex, NodeIndex};
use topology::TopologyGraph;

/// Trait for routing engines. `sim-core` does not depend on concrete
/// routing implementations (ECMP, adaptive), it only knows this trait.
pub trait Router {
    /// Returns the sequence of edges to traverse from `src` to `dst`.
    /// `flow_id` is used by ECMP for consistent hashing.
    fn route(
        &self,
        graph: &TopologyGraph,
        src: NodeIndex,
        dst: NodeIndex,
        flow_id: u64,
    ) -> Option<Vec<EdgeIndex>>;
}