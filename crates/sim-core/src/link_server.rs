//! Directional link: FIFO buffer + server that serializes fleets.

use std::collections::VecDeque;
use petgraph::graph::NodeIndex;

use crate::flit::FlitRef;

pub type NodeId = NodeIndex;

#[derive(Debug, Clone, Default)]
pub struct LinkStats {
    pub flits_forwarded: u64,
    pub flits_dropped: u64,
    pub bytes_forwarded: u64,
    pub total_busy_ns: u64,
    pub max_queue_depth: usize,
    pub queue_depth_samples: u64,
    pub queue_depth_sum: u64,
}

impl LinkStats {
    pub fn avg_queue_depth(&self) -> f64 {
        if self.queue_depth_samples == 0 { 0.0 }
        else { self.queue_depth_sum as f64 / self.queue_depth_samples as f64 }
    }
    pub fn utilization(&self, total_time_ns: u64) -> f64 {
        if total_time_ns == 0 { 0.0 }
        else { self.total_busy_ns as f64 / total_time_ns as f64 }
    }
}

#[derive(Debug, Clone)]
pub struct DirectionalLink {
    pub from: NodeId,
    pub to: NodeId,
    pub bandwidth_gbps: f64,
    pub latency_ns: u64,
    pub capacity: usize,
    pub buffer: VecDeque<FlitRef>,
    pub busy_until_ns: u64,
    pub stats: LinkStats,
}

impl DirectionalLink {
    pub fn new(from: NodeId, to: NodeId, bw: f64, lat: u64, cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            from, to,
            bandwidth_gbps: bw,
            latency_ns: lat,
            capacity: cap,
            buffer: VecDeque::with_capacity(cap),
            busy_until_ns: 0,
            stats: LinkStats::default(),
        }
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.capacity
    }

    // Time to drop the flit to the link (in ns).
    #[inline]
    pub fn serialization_ns(&self, flit_size_bytes: u32) -> u64 {
        // bits / bits_per_ns = ns ; bits_per_ns = bw_gbps
        let ns = (flit_size_bytes as f64) * 8.0 / self.bandwidth_gbps;
        ns.ceil() as u64
    }

    pub fn record_queue_sample(&mut self) {
        let depth = self.buffer.len();
        self.stats.queue_depth_samples += 1;
        self.stats.queue_depth_sum += depth as u64;
        if depth > self.stats.max_queue_depth {
            self.stats.max_queue_depth = depth;
        }
    }
}