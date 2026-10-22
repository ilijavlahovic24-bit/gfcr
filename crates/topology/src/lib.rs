pub mod config;
pub mod fat_tree;
pub mod graph;

pub use config::{LinkParams, TopologyConfig, TopologyKind, TopologyError};
pub use graph::{Link, Node, NodeId, NodeKind, TopologyGraph};