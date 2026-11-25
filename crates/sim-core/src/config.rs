//! Simulation configuration.

use crate::flit::{Fidelity, FlitConfig};

#[derive(Debug, Clone)]
pub struct SimConfig {
    pub fidelity: Fidelity,
    pub flit: FlitConfig,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            fidelity: Fidelity::Flow,
            flit: FlitConfig::default(),
        }
    }
}