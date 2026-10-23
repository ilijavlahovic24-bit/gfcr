//! The discrete-event simulation engine.
//!
//! See lib.rs for the modelling overview. This module wires together:
//! - the event queue,
//! - per-link active-flow bookkeeping (full-duplex),
//! - the fluid rate computation (min-over-path fair share),
//! - per-flow progress projection and completion scheduling.

use std::collections::HashMap;

use petgraph::graph::{EdgeIndex, NodeIndex};
use topology::TopologyGraph;

use crate::event::{EventKind, EventQueue};
use crate::flow::{ActiveFlow, Flow, FlowId, FlowResult};
use crate::link_model::gbps_to_bytes_per_ns;
use crate::router::Router;

pub struct SimulationResult {
    pub completed_flows: Vec<FlowResult>,
    /// Wall-clock simulation time at which the last event fired.
    pub sim_time_ns: u64,
    pub utilization_history: Vec<LinkUtilizationRecord>,
}

#[derive(Debug, Clone)]
pub struct LinkUtilizationRecord {
    pub time_ns: u64,
    pub link_id: usize, // EdgeIndex
    pub utilization_pct: f64,
}

pub struct Simulator {
    graph: TopologyGraph,
    router: Box<dyn Router>,

    /// Pre-computed link parameters, indexed by EdgeIndex.
    link_bw_bpns: HashMap<EdgeIndex, f64>,
    link_latency_ns: HashMap<EdgeIndex, u64>,

    /// State.
    active_flows: HashMap<FlowId, ActiveFlow>,
    /// Tracks active flows per directed link (src, dst) to model full-duplex.
    link_active_flows: HashMap<(NodeIndex, NodeIndex), Vec<FlowId>>,

    events: EventQueue,
    now_ns: u64,
    completed_flows: Vec<FlowResult>,
    utilization_history: Vec<LinkUtilizationRecord>,
}

impl Simulator {
    pub fn new(graph: TopologyGraph, input_flows: Vec<Flow>, router: Box<dyn Router>) -> Self {
        let mut link_bw_bpns = HashMap::new();
        let mut link_latency_ns = HashMap::new();
        for eid in graph.inner().edge_indices() {
            let link = &graph.inner()[eid];
            link_bw_bpns.insert(eid, gbps_to_bytes_per_ns(link.bandwidth_gbps));
            link_latency_ns.insert(eid, link.latency_ns);
        }

        let mut sim = Self {
            graph,
            router,
            link_bw_bpns,
            link_latency_ns,
            active_flows: HashMap::new(),
            link_active_flows: HashMap::new(),
            events: EventQueue::new(),
            now_ns: 0,
            completed_flows: Vec::new(),
            utilization_history: Vec::new(),
        };

        for flow in input_flows {
            let t = flow.start_time_ns;
            sim.events.push(t, EventKind::FlowArrival { flow });
        }

        sim
    }

    pub fn run(mut self) -> SimulationResult {
        while let Some(event) = self.events.pop() {
            self.now_ns = event.time_ns;
            match event.kind {
                EventKind::FlowArrival { flow } => self.on_flow_arrival(flow),
                EventKind::FlowComplete {
                    flow_id,
                    expected_ns,
                } => self.on_flow_complete(flow_id, expected_ns),
            }
        }

        SimulationResult {
            completed_flows: self.completed_flows,
            sim_time_ns: self.now_ns,
            utilization_history: self.utilization_history, // ISPRAVLJENO
        }
    }

    fn on_flow_arrival(&mut self, flow: Flow) {
        // 1. Calculate current load on all links (before adding the new flow)
        let mut link_load: HashMap<EdgeIndex, f64> = HashMap::new();
        for (eid, bw) in &self.link_bw_bpns {
            let (a, b) = self.graph.inner().edge_endpoints(*eid).unwrap();
            let flows_ab = self.link_active_flows.get(&(a, b)).map(|v| v.as_slice()).unwrap_or(&[]);
            let flows_ba = self.link_active_flows.get(&(b, a)).map(|v| v.as_slice()).unwrap_or(&[]);

            let mut total_rate = 0.0;
            for fid in flows_ab.iter().chain(flows_ba.iter()) {
                if let Some(f) = self.active_flows.get(fid) {
                    // If the flow has finished transmitting, it doesn't load the link anymore
                    if f.tx_complete_ns.is_none() || self.now_ns < f.tx_complete_ns.unwrap() {
                        total_rate += f.current_rate_bpns;
                    }
                }
            }
            link_load.insert(*eid, total_rate / bw);
        }

        // 2. Delegate path selection to the injected router with current load info.
        let Some(path_edges) = self.router.route(&self.graph, flow.src, flow.dst, flow.id, &link_load) else {
            return;
        };

        // Reconstruct directed edges to properly track full-duplex links.
        let mut cur = flow.src;
        let mut directed_edges = Vec::with_capacity(path_edges.len());
        for &eid in &path_edges {
            let (a, b) = self.graph.inner().edge_endpoints(eid).unwrap();
            let next = if a == cur { b } else { a };
            directed_edges.push((cur, next));
            cur = next;
        }

        let path_latency_ns: u64 = path_edges
            .iter()
            .map(|e| *self.link_latency_ns.get(e).unwrap_or(&0))
            .sum();

        let active = ActiveFlow {
            id: flow.id,
            src: flow.src,
            dst: flow.dst,
            size_bytes: flow.size_bytes,
            remaining_bytes: flow.size_bytes as f64,
            start_time_ns: self.now_ns,
            last_update_ns: self.now_ns,
            tx_complete_ns: None,
            path_edges: path_edges.clone(),
            directed_edges: directed_edges.clone(),
            path_latency_ns,
            current_rate_bpns: 0.0,
            expected_complete_ns: None,
        };

        for dir_edge in &directed_edges {
            self.link_active_flows
                .entry(*dir_edge)
                .or_default()
                .push(flow.id);
        }

        self.active_flows.insert(flow.id, active);
        self.recompute_and_reschedule();
    }

    fn on_flow_complete(&mut self, flow_id: FlowId, expected_ns: u64) {
        let Some(flow) = self.active_flows.get(&flow_id) else {
            return;
        };
        if flow.expected_complete_ns != Some(expected_ns) {
            return;
        }

        let flow = self.active_flows.remove(&flow_id).unwrap();

        for dir_edge in &flow.directed_edges {
            if let Some(list) = self.link_active_flows.get_mut(dir_edge) {
                list.retain(|id| *id != flow_id);
            }
        }

        let end_time_ns = self.now_ns;
        let fct_ns = end_time_ns - flow.start_time_ns;
        self.completed_flows.push(FlowResult {
            id: flow.id,
            src: flow.src,
            dst: flow.dst,
            size_bytes: flow.size_bytes,
            start_time_ns: flow.start_time_ns,
            end_time_ns,
            fct_ns,
        });

        self.recompute_and_reschedule();
    }

    fn recompute_and_reschedule(&mut self) {
        let now = self.now_ns;

        for flow in self.active_flows.values_mut() {
            let elapsed = now.saturating_sub(flow.last_update_ns);
            if elapsed == 0 {
                continue;
            }
            let delta = (elapsed as f64) * flow.current_rate_bpns;
            if flow.remaining_bytes > delta {
                flow.remaining_bytes -= delta;
            } else {
                let rate = flow.current_rate_bpns.max(f64::EPSILON);
                let exact_tx_ns =
                    flow.last_update_ns + (flow.remaining_bytes / rate).ceil() as u64;
                flow.tx_complete_ns = Some(exact_tx_ns);
                flow.remaining_bytes = 0.0;
            }
            flow.last_update_ns = now;
        }

        let mut new_rates: Vec<(FlowId, f64)> = Vec::with_capacity(self.active_flows.len());
        for (fid, flow) in &self.active_flows {
            let mut rate = f64::INFINITY;
            for (&eid, dir_edge) in flow.path_edges.iter().zip(flow.directed_edges.iter()) {
                let n = self
                    .link_active_flows
                    .get(dir_edge)
                    .map(|v| v.len() as f64)
                    .unwrap_or(0.0);
                if n > 0.0 {
                    let fair = self.link_bw_bpns[&eid] / n;
                    if fair < rate {
                        rate = fair;
                    }
                }
            }
            if rate.is_finite() && rate > 0.0 {
                new_rates.push((*fid, rate));
            }
        }
        for (fid, rate) in new_rates {
            if let Some(flow) = self.active_flows.get_mut(&fid) {
                flow.current_rate_bpns = rate;
            }
        }

        let mut to_schedule: Vec<(u64, FlowId)> = Vec::with_capacity(self.active_flows.len());
        for flow in self.active_flows.values_mut() {
            let delivery = if let Some(tx) = flow.tx_complete_ns {
                tx + flow.path_latency_ns
            } else if flow.current_rate_bpns > 0.0 {
                let tx_remaining =
                    (flow.remaining_bytes / flow.current_rate_bpns).ceil() as u64;
                now + tx_remaining + flow.path_latency_ns
            } else {
                continue;
            };
            let delivery = delivery.max(now);
            flow.expected_complete_ns = Some(delivery);
            to_schedule.push((delivery, flow.id));
        }

        for (delivery, fid) in to_schedule {
            self.events.push(
                delivery,
                EventKind::FlowComplete {
                    flow_id: fid,
                    expected_ns: delivery,
                },
            );
        }

        // --- Step 4: Record link utilization.
        for (dir_edge, flow_ids) in &self.link_active_flows {
            if flow_ids.is_empty() { continue; }

            let (a, b) = *dir_edge;
            let eid = self.graph.inner().find_edge(a, b).unwrap();
            let bw = self.link_bw_bpns[&eid];

            let mut total_rate = 0.0;
            for fid in flow_ids {
                if let Some(flow) = self.active_flows.get(fid) {
                    if flow.tx_complete_ns.is_none() || now < flow.tx_complete_ns.unwrap() {
                        total_rate += flow.current_rate_bpns;
                    }
                }
            }

            let utilization_pct = (total_rate / bw) * 100.0;
            self.utilization_history.push(LinkUtilizationRecord {
                time_ns: now,
                link_id: eid.index(),
                utilization_pct,
            });
        }
    }
}