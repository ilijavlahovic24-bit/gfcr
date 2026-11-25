pub mod clock;
pub mod engine;
pub mod event;
pub mod flow;
pub mod link_model;
pub mod path;
pub mod router;

pub mod config;
pub mod flit;
pub mod flit_engine;
pub mod link_server;
pub mod metrics;
pub mod report;

// ─── Flow-level (existing) ───
pub use engine::{LinkUtilizationRecord, SimulationResult, Simulator};
pub use flow::{Flow, FlowId, FlowResult};

// ─── Flit-level (new) ───
// NOTE: `flit::Flow` and `flit::FlowId` are intentionally not re-exported at
// the crate root to avoid clashing with the flow-level types above. Use the
// module path `sim_core::flit::Flow` / `sim_core::flit::FlowId` when working
// with flit-level scenarios. Same for `flit_engine::FlowResult`.
pub use config::SimConfig;
pub use flit::{Arbitration, Fidelity, FlitConfig, Injection, OverflowPolicy};
pub use flit_engine::{FlitEngine, SimResult};

// ─── Router ───
pub use router::Router;

// ─── Topology re-export ───
pub use topology::TopologyGraph;