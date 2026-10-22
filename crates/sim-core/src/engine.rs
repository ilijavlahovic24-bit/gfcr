//! The discrete-event simulation engine.
//!
//! See lib.rs for the modeling overview. This module wires together:
//! - the event queue,
//! - per-link active-flow bookkeeping,
//! - the fluid rate computation (min-over-path fair share),
//! - per-flow progress projection and completion scheduling.

use std::collections::HashMap;

use petgraph::graph::EdgeIndex;
use topology::TopologyGraph;

use crate::event::{EventKind, EventQueue};
use crate::flow::{ActiveFlow, Flow, FlowId, FlowResult};
use crate::link_model::gbps_to_bytes_per_ns;
use crate::path::shortest_path;

pub struct SimulationResult {
    pub completed_flows: Vec<FlowResult>,
    /// Wall-clock simulation time at which the last event fired.
    pub sim_time_ns: u64,
}

pub struct Simulator {
    graph: TopologyGraph,

    /// Pre-computed link parameters, indexed by EdgeIndex.
    link_bw_bpns: HashMap<EdgeIndex, f64>,
    link_latency_ns: HashMap<EdgeIndex, u64>,

    /// State.
    active_flows: HashMap<FlowId, ActiveFlow>,
    link_active_flows: HashMap<EdgeIndex, Vec<FlowId>>,

    events: EventQueue,
    now_ns: u64,
    completed_flows: Vec<FlowResult>,
}

impl Simulator {
    pub fn new(graph: TopologyGraph, input_flows: Vec<Flow>) -> Self {
        let mut link_bw_bpns = HashMap::new();
        let mut link_latency_ns = HashMap::new();
        for eid in graph.inner().edge_indices() {
            let link = &graph.inner()[eid];
            link_bw_bpns.insert(eid, gbps_to_bytes_per_ns(link.bandwidth_gbps));
            link_latency_ns.insert(eid, link.latency_ns);
        }

        let mut sim = Self {
            graph,
            link_bw_bpns,
            link_latency_ns,
            active_flows: HashMap::new(),
            link_active_flows: HashMap::new(),
            events: EventQueue::new(),
            now_ns: 0,
            completed_flows: Vec::new(),
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
        }
    }

    fn on_flow_arrival(&mut self, flow: Flow) {
        let Some(path_edges) = shortest_path(&self.graph, flow.src, flow.dst) else {
            // Unreachable destination — drop the flow. In MVP topologies
            // (fat-tree) this never happens; validated by is_connected test.
            return;
        };

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
            path_latency_ns,
            current_rate_bpns: 0.0,
            expected_complete_ns: None,
        };

        for eid in &path_edges {
            self.link_active_flows
                .entry(*eid)
                .or_default()
                .push(flow.id);
        }

        self.active_flows.insert(flow.id, active);
        self.recompute_and_reschedule();
    }

    fn on_flow_complete(&mut self, flow_id: FlowId, expected_ns: u64) {
        // Ignore if already completed.
        let Some(flow) = self.active_flows.get(&flow_id) else {
            return;
        };
        // Ignore if the flow has been rescheduled since this event was pushed.
        if flow.expected_complete_ns != Some(expected_ns) {
            return;
        }

        let flow = self.active_flows.remove(&flow_id).unwrap();
        for eid in &flow.path_edges {
            if let Some(list) = self.link_active_flows.get_mut(eid) {
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

    /// Core of the fluid model. Called after every state change (arrival or
    /// completion). Steps:
    ///   1. Advance every active flow's `remaining_bytes` using its OLD rate.
    ///   2. Compute NEW rates (min over path of link fair share).
    ///   3. Project each flow's delivery time and schedule a FlowComplete.
    ///
    /// Cost is O(F * L) per event, where F = active flows and L = path length.
    /// Fine for MVP sizes. Progressive-filling upgrade would touch only the
    /// flows affected by the changed links, but that's an optimization.
    fn recompute_and_reschedule(&mut self) {
        let now = self.now_ns;

        // --- Step 1: advance progress using the old rate.
        for flow in self.active_flows.values_mut() {
            let elapsed = now.saturating_sub(flow.last_update_ns);
            if elapsed == 0 {
                continue;
            }
            let delta = (elapsed as f64) * flow.current_rate_bpns;
            if flow.remaining_bytes > delta {
                flow.remaining_bytes -= delta;
            } else {
                // Transmission completed during this interval.
                let rate = flow.current_rate_bpns.max(f64::EPSILON);
                let exact_tx_ns =
                    flow.last_update_ns + (flow.remaining_bytes / rate).ceil() as u64;
                flow.tx_complete_ns = Some(exact_tx_ns);
                flow.remaining_bytes = 0.0;
            }
            flow.last_update_ns = now;
        }

        // --- Step 2: compute new rates. Two-pass to keep the borrow checker
        // happy: collect first, then apply.
        let mut new_rates: Vec<(FlowId, f64)> = Vec::with_capacity(self.active_flows.len());
        for (fid, flow) in &self.active_flows {
            let mut rate = f64::INFINITY;
            for eid in &flow.path_edges {
                let n = self
                    .link_active_flows
                    .get(eid)
                    .map(|v| v.len() as f64)
                    .unwrap_or(0.0);
                if n > 0.0 {
                    let fair = self.link_bw_bpns[eid] / n;
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

        // --- Step 3: project delivery and schedule.
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
            let delivery = delivery.max(now); // defensive clamp
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
    }
}