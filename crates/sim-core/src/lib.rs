//! Discrete-event GPU fabric simulator (flow-level fidelity).
//!
//! Model summary (see ADR-002 for the flow-vs-flit decision, and the
//! in-crate doc for the MVP fluid approximation):
//!
//! - Flows advance as continuous fluid through all links on their path
//!   simultaneously.
//! - Per-link fair share = link_bandwidth / number_of_active_flows_on_link.
//! - Flow rate = min over path of per-link fair share (MVP approximation
//!   of true max-min fairness; progressive filling is a future upgrade).
//! - Buffers are NOT modelled explicitly. Fluid is smooth, so no queueing
//!   occurs. Congestion effects appear as rate reduction, which is
//!   sufficient for FCT and utilization metrics at flow-level.
//! - Flow Completion Time (FCT) = tx_time + path_latency, where
//!   tx_time = size / rate and path_latency is the sum of link
//!   propagation delays along the path.

pub mod clock;
pub mod engine;
pub mod event;
pub mod flow;
pub mod link_model;
pub mod path;

pub use engine::{SimulationResult, Simulator};
pub use flow::{Flow, FlowId, FlowResult};
pub use topology::TopologyGraph;
pub mod router;
pub use router::Router;