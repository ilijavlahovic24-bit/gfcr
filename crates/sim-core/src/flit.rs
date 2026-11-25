//! Flit-level types and configuration.

use petgraph::graph::NodeIndex;

pub type NodeId = NodeIndex;
pub type FlowId = u64;

#[derive(Debug, Clone, Copy)]
pub struct FlitConfig {
    pub flit_size_bytes: u32,
    pub arbitration: Arbitration,
    pub overflow: OverflowPolicy,
    pub injection: Injection,
}

impl Default for FlitConfig {
    fn default() -> Self {
        Self {
            flit_size_bytes: 64,
            arbitration: Arbitration::Fifo,
            overflow: OverflowPolicy::DropTail,
            injection: Injection::Paced,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arbitration {
    /// Strict first-in-first-out across all flits in a buffer.
    Fifo,
    /// Round-robin per flow (TODO).
    RoundRobin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// Drop the incoming flit when the buffer is full.
    DropTail,
    /// Propagate backpressure to the upstream link instead of dropping (TODO).
    Backpressure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Injection {
    /// Source emits flits at the rate of the first link (realistic for a NIC).
    Paced,
    /// All flits are injected at `t = start_ns` (burst).
    Burst,
}

/// Transport fidelity selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fidelity {
    /// Fluid approximation (existing flow-level engine).
    Flow,
    /// Discrete flit-level transport (`flit_engine`).
    Flit,
}

impl Default for Fidelity {
    fn default() -> Self {
        Self::Flow
    }
}

/// Flow input to the simulation.
#[derive(Debug, Clone)]
pub struct Flow {
    pub id: FlowId,
    pub src: NodeId,
    pub dst: NodeId,
    pub size_bytes: u64,
    pub start_ns: u64,
}

/// Lightweight reference to a flit sitting in a buffer. Does not carry the
/// full flit; only the identifiers the engine needs to schedule it.
#[derive(Debug, Clone, Copy)]
pub struct FlitRef {
    pub flow_id: FlowId,
    pub flit_seq: u32,
    pub hop: usize,
}