//! Simulation time model. Everything is nanoseconds, u64.

pub type TimeNs = u64;

pub const NS_PER_US: TimeNs = 1_000;
pub const NS_PER_MS: TimeNs = 1_000_000;
pub const NS_PER_SEC: TimeNs = 1_000_000_000;