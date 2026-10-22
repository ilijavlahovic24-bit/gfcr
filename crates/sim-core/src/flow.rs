//! Flow types.
//!
//! `Flow` is the input description (what traffic-gen produces). `ActiveFlow`
//! is the simulator's mutable view of a flow currently in the network.
//! `FlowResult` is the record emitted when a flow completes.

use petgraph::graph::{EdgeIndex, NodeIndex};

pub type FlowId = u64;

/// Input description of a flow, produced by traffic-gen or tests.
#[derive(Debug, Clone)]
pub struct Flow {
    pub id: FlowId,
    pub src: NodeIndex,
    pub dst: NodeIndex,
    pub size_bytes: u64,
    pub start_time_ns: u64,
}

/// Simulator's view of a flow currently in the network.
#[derive(Debug)]
pub(crate) struct ActiveFlow {
    pub id: FlowId,
    pub src: NodeIndex,
    pub dst: NodeIndex,
    pub size_bytes: u64,

    /// Fluid still to be transmitted from the source, in bytes. Decreases
    /// over time as the flow advances at `current_rate_bpns`.
    pub remaining_bytes: f64,

    pub start_time_ns: u64,

    /// Timestamp of the last time we "committed" progress to
    /// `remaining_bytes`. On every event that touches this flow we advance
    /// remaining by `(now - last_update) * rate`.
    pub last_update_ns: u64,

    /// The exact time the last bit left the source, if transmission has
    /// already completed. Once set, delivery time is
    /// `tx_complete_ns + path_latency_ns` and is no longer affected by
    /// rate changes.
    pub tx_complete_ns: Option<u64>,

    pub path_edges: Vec<EdgeIndex>,
    pub path_latency_ns: u64,

    /// Current rate in bytes per nanosecond. Recomputed on every state change.
    pub current_rate_bpns: f64,

    /// The delivery time this flow is currently projected to finish at.
    /// Used for staleness detection of `FlowComplete` events.
    pub expected_complete_ns: Option<u64>,
}

/// Output record for one completed flow.
#[derive(Debug, Clone)]
pub struct FlowResult {
    pub id: FlowId,
    pub src: NodeIndex,
    pub dst: NodeIndex,
    pub size_bytes: u64,
    pub start_time_ns: u64,
    pub end_time_ns: u64,
    pub fct_ns: u64,
}