use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::{dragonfly, fat_tree};
use crate::graph::{Link, LinkClass, TopologyGraph};

#[derive(Debug, Deserialize)]
pub struct TopologyConfig {
    pub topology: TopologyKind,
    pub links: LinkParams,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LinkParamsYaml {
    pub terminal_bw:  Option<f64>,
    pub terminal_lat: Option<u64>,
    pub terminal_buf: Option<usize>,
    pub local_bw:     Option<f64>,
    pub local_lat:    Option<u64>,
    pub local_buf:    Option<usize>,
    pub global_bw:    Option<f64>,
    pub global_lat:   Option<u64>,
    pub global_buf:   Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TopologyKind {
    FatTree { k: usize },
    Dragonfly {
        a: usize,
        p: usize,
        h: usize,
        g: usize,
        #[serde(default)]
        link: Option<LinkParamsYaml>,
    },
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
            class: LinkClass::Generic
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
            TopologyKind::Dragonfly { a, p, h, g, link } => {
                let d = dragonfly::LinkParams::default();
                let lp = match link {
                    Some(y) => dragonfly::LinkParams {
                        terminal_bw:  y.terminal_bw .unwrap_or(d.terminal_bw),
                        terminal_lat: y.terminal_lat.unwrap_or(d.terminal_lat),
                        terminal_buf: y.terminal_buf.unwrap_or(d.terminal_buf),
                        local_bw:     y.local_bw    .unwrap_or(d.local_bw),
                        local_lat:    y.local_lat   .unwrap_or(d.local_lat),
                        local_buf:    y.local_buf   .unwrap_or(d.local_buf),
                        global_bw:    y.global_bw   .unwrap_or(d.global_bw),
                        global_lat:   y.global_lat  .unwrap_or(d.global_lat),
                        global_buf:   y.global_buf  .unwrap_or(d.global_buf),
                    },
                    None => d,
                };
                dragonfly::generate(
                    dragonfly::DragonflyParams { a: *a, p: *p, h: *h, g: *g },
                    lp,
                )
            }
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
    #[error("dragonfly: {0}")]
    Dragonfly(String),
}