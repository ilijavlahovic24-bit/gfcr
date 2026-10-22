use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::fat_tree;
use crate::graph::{Link, TopologyGraph};

#[derive(Debug, Deserialize)]
pub struct TopologyConfig {
    pub topology: TopologyKind,
    pub links: LinkParams,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TopologyKind {
    FatTree { k: usize },
}

#[derive(Debug, Deserialize, Clone)]
pub struct LinkParams {
    pub bandwidth_gbps: f64,
    pub latency_ns: u64,
    pub buffer_size: usize,
}

impl LinkParams {
    pub fn to_link(&self) -> Link {
        Link {
            bandwidth_gbps: self.bandwidth_gbps,
            latency_ns: self.latency_ns,
            buffer_size: self.buffer_size,
        }
    }
}

impl TopologyConfig {
    pub fn load_from_file(path: &Path) -> Result<Self, TopologyError> {
        let text = fs::read_to_string(path)?;
        let cfg: TopologyConfig = serde_yaml::from_str(&text)?;
        Ok(cfg)
    }

    pub fn build(&self) -> Result<TopologyGraph, TopologyError> {
        match &self.topology {
            TopologyKind::FatTree { k } => fat_tree::generate(*k, self.links.to_link()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TopologyError {
    #[error("k mus be even i >= 2, : {0}")]
    InvalidK(usize),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}