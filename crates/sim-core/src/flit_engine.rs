//! A flit-level discrete-event simulation engine.

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::Arc;

use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::flit::{FlitConfig, FlitRef, Flow, FlowId, Injection};
use crate::link_server::{DirectionalLink, LinkStats};
use crate::TopologyGraph;

type NodeId = NodeIndex;

// ─────────── Events ───────────

#[derive(Debug, Clone, Copy)]
enum FlitEventKind {
    FlowArrival {
        flow_id: FlowId,
    },
    /// A flit tries to enter the buffer of link (path[hop], path[hop+1]).
    FlitEnqueue {
        flow_id: FlowId,
        flit_seq: u32,
        hop: usize,
    },
    /// Serialization on link (path[hop], path[hop+1]) is done; the flit
    /// propagates towards the next hop.
    FlitSerialized {
        flow_id: FlowId,
        flit_seq: u32,
        hop: usize,
    },
}

#[derive(Debug, Clone, Copy)]
struct FlitEvent {
    at_ns: u64,
    seq: u64,
    kind: FlitEventKind,
}

impl PartialEq for FlitEvent {
    fn eq(&self, o: &Self) -> bool {
        self.at_ns == o.at_ns && self.seq == o.seq
    }
}
impl Eq for FlitEvent {}
impl PartialOrd for FlitEvent {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for FlitEvent {
    fn cmp(&self, o: &Self) -> Ordering {
        o.at_ns.cmp(&self.at_ns).then(o.seq.cmp(&self.seq))
    }
}

// ─────────── Per-flow state ───────────

#[derive(Debug, Clone)]
struct FlowState {
    flow: Flow,
    path: Vec<NodeId>,
    total_flits: u32,
    delivered: u32,
    dropped: u32,
    end_ns: Option<u64>,
}

// ─────────── Engine ───────────

pub struct FlitEngine {
    topology: Arc<TopologyGraph>,
    flows: Vec<Flow>,
    cfg: FlitConfig,
    links: HashMap<(NodeId, NodeId), DirectionalLink>,
    flow_states: HashMap<FlowId, FlowState>,
    events: BinaryHeap<Reverse<FlitEvent>>,
    event_seq: u64,
    now_ns: u64,
    last_event_ns: u64,
}

impl FlitEngine {
    pub fn new(topology: Arc<TopologyGraph>, flows: Vec<Flow>, cfg: FlitConfig) -> Self {
        Self {
            topology,
            flows,
            cfg,
            links: HashMap::new(),
            flow_states: HashMap::new(),
            events: BinaryHeap::new(),
            event_seq: 0,
            now_ns: 0,
            last_event_ns: 0,
        }
    }

    // ─── Helpers ───

    fn schedule(&mut self, at_ns: u64, kind: FlitEventKind) {
        self.events.push(Reverse(FlitEvent {
            at_ns,
            seq: self.event_seq,
            kind,
        }));
        self.event_seq += 1;
    }

    /// Create TWO directed links for every undirected edge (full-duplex).
    fn init_links(&mut self) {
        let inner = self.topology.inner();
        for e in inner.edge_indices() {
            let (u, v) = inner.edge_endpoints(e).unwrap();
            let w = inner.edge_weight(e).unwrap();
            let cap = w.buffer_size.max(1);
            self.links.insert(
                (u, v),
                DirectionalLink::new(u, v, w.bandwidth_gbps, w.latency_ns, cap),
            );
            self.links.insert(
                (v, u),
                DirectionalLink::new(v, u, w.bandwidth_gbps, w.latency_ns, cap),
            );
        }
    }

    /// BFS shortest path. Replace with your own router if you have one.
    fn compute_path(&self, src: NodeId, dst: NodeId) -> Vec<NodeId> {
        if src == dst {
            return vec![src];
        }
        let inner = self.topology.inner();
        let mut prev: HashMap<NodeId, NodeId> = HashMap::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut q: VecDeque<NodeId> = VecDeque::new();
        visited.insert(src);
        q.push_back(src);
        while let Some(n) = q.pop_front() {
            if n == dst {
                break;
            }
            for e in inner.edges(n) {
                let other = if e.source() == n { e.target() } else { e.source() };
                if visited.insert(other) {
                    prev.insert(other, n);
                    q.push_back(other);
                }
            }
        }
        if !visited.contains(&dst) {
            return vec![];
        }
        let mut path = vec![dst];
        let mut cur = dst;
        while cur != src {
            cur = prev[&cur];
            path.push(cur);
        }
        path.reverse();
        path
    }

    // ─── Handlers ───

    fn on_flow_arrival(&mut self, flow_id: FlowId) {
        let flow = match self.flows.iter().find(|f| f.id == flow_id) {
            Some(f) => f.clone(),
            None => return,
        };
        let path = self.compute_path(flow.src, flow.dst);
        if path.len() < 2 {
            return;
        }
        let flit_size = self.cfg.flit_size_bytes as u64;
        let total_flits = ((flow.size_bytes + flit_size - 1) / flit_size) as u32;

        self.flow_states.insert(
            flow_id,
            FlowState {
                flow: flow.clone(),
                path: path.clone(),
                total_flits,
                delivered: 0,
                dropped: 0,
                end_ns: None,
            },
        );

        let t0 = self.now_ns;
        match self.cfg.injection {
            Injection::Burst => {
                for seq in 0..total_flits {
                    self.schedule(
                        t0,
                        FlitEventKind::FlitEnqueue {
                            flow_id,
                            flit_seq: seq,
                            hop: 0,
                        },
                    );
                }
            }
            Injection::Paced => {
                let first = &self.links[&(path[0], path[1])];
                let ser = first.serialization_ns(self.cfg.flit_size_bytes);
                for seq in 0..total_flits {
                    self.schedule(
                        t0 + (seq as u64) * ser,
                        FlitEventKind::FlitEnqueue {
                            flow_id,
                            flit_seq: seq,
                            hop: 0,
                        },
                    );
                }
            }
        }
    }

    fn on_flit_enqueue(&mut self, flow_id: FlowId, flit_seq: u32, hop: usize) {
        let path = match self.flow_states.get(&flow_id) {
            Some(s) => s.path.clone(),
            None => return,
        };
        if hop + 1 >= path.len() {
            return;
        }
        let link_key = (path[hop], path[hop + 1]);

        let dropped = {
            let link = match self.links.get_mut(&link_key) {
                Some(l) => l,
                None => return,
            };
            link.record_queue_sample();
            if link.is_full() {
                true
            } else {
                link.buffer.push_back(FlitRef {
                    flow_id,
                    flit_seq,
                    hop,
                });
                false
            }
        };

        if dropped {
            if let Some(link) = self.links.get_mut(&link_key) {
                link.stats.flits_dropped += 1;
            }
            if let Some(state) = self.flow_states.get_mut(&flow_id) {
                state.dropped += 1;
                if state.delivered + state.dropped == state.total_flits {
                    state.end_ns = Some(self.now_ns);
                }
            }
            return;
        }

        self.kick_link(link_key);
    }

    /// If the link is idle and the buffer is non-empty, pop one flit and
    /// schedule its serialization completion.
    fn kick_link(&mut self, link_key: (NodeId, NodeId)) {
        let now = self.now_ns;
        let flit_size = self.cfg.flit_size_bytes;

        let scheduled = {
            let link = match self.links.get_mut(&link_key) {
                Some(l) => l,
                None => return,
            };
            if link.buffer.is_empty() {
                return;
            }
            if link.busy_until_ns > now {
                return;
            }

            let flit = link.buffer.pop_front().unwrap();
            let ser = link.serialization_ns(flit_size);
            link.busy_until_ns = now + ser;
            link.stats.flits_forwarded += 1;
            link.stats.bytes_forwarded += flit_size as u64;
            link.stats.total_busy_ns += ser;
            link.record_queue_sample();

            (flit, link.busy_until_ns)
        };

        let (flit, done_at) = scheduled;
        self.schedule(
            done_at,
            FlitEventKind::FlitSerialized {
                flow_id: flit.flow_id,
                flit_seq: flit.flit_seq,
                hop: flit.hop,
            },
        );
    }

    fn on_flit_serialized(&mut self, flow_id: FlowId, flit_seq: u32, hop: usize) {
        let (path, latency) = match self.flow_states.get(&flow_id) {
            Some(s) => {
                let link = &self.links[&(s.path[hop], s.path[hop + 1])];
                (s.path.clone(), link.latency_ns)
            }
            None => return,
        };

        let arrive_at = self.now_ns + latency;
        let next_node = path[hop + 1];
        let dst_node = path[path.len() - 1];

        if next_node == dst_node {
            // Flit reached its destination.
            if let Some(state) = self.flow_states.get_mut(&flow_id) {
                state.delivered += 1;
                if state.delivered + state.dropped == state.total_flits {
                    state.end_ns = Some(arrive_at);
                }
            }
        } else {
            self.schedule(
                arrive_at,
                FlitEventKind::FlitEnqueue {
                    flow_id,
                    flit_seq,
                    hop: hop + 1,
                },
            );
        }

        // Link (path[hop], path[hop+1]) is now free (`busy_until_ns == now`);
        // start serializing the next flit in its buffer, if any.
        self.kick_link((path[hop], path[hop + 1]));
    }

    // ─── Main loop ───

    pub fn run(&mut self) -> SimResult {
        self.init_links();

        // Schedule all flow arrivals.
        let arrivals: Vec<(u64, FlowId)> = self
            .flows
            .iter()
            .map(|f| (f.start_ns, f.id))
            .collect();
        for (t, id) in arrivals {
            self.schedule(t, FlitEventKind::FlowArrival { flow_id: id });
        }

        while let Some(Reverse(ev)) = self.events.pop() {
            self.now_ns = ev.at_ns;
            self.last_event_ns = self.now_ns;
            match ev.kind {
                FlitEventKind::FlowArrival { flow_id } => self.on_flow_arrival(flow_id),
                FlitEventKind::FlitEnqueue {
                    flow_id,
                    flit_seq,
                    hop,
                } => self.on_flit_enqueue(flow_id, flit_seq, hop),
                FlitEventKind::FlitSerialized {
                    flow_id,
                    flit_seq,
                    hop,
                } => self.on_flit_serialized(flow_id, flit_seq, hop),
            }
        }

        self.build_result()
    }

    fn build_result(&self) -> SimResult {
        let total_time = self.last_event_ns;

        let flows: Vec<FlowResult> = self
            .flow_states
            .values()
            .map(|s| {
                let fct = s.end_ns.map(|e| e.saturating_sub(s.flow.start_ns));
                FlowResult {
                    flow_id: s.flow.id,
                    src: s.flow.src,
                    dst: s.flow.dst,
                    size_bytes: s.flow.size_bytes,
                    start_ns: s.flow.start_ns,
                    end_ns: s.end_ns,
                    fct_ns: fct,
                    total_flits: s.total_flits,
                    delivered: s.delivered,
                    dropped: s.dropped,
                    success: s.delivered == s.total_flits,
                }
            })
            .collect();

        let links: HashMap<(NodeId, NodeId), LinkStats> = self
            .links
            .iter()
            .map(|(k, v)| (*k, v.stats.clone()))
            .collect();

        SimResult {
            flows,
            links,
            total_time_ns: total_time,
            flit_size_bytes: self.cfg.flit_size_bytes,
        }
    }
}

// ─────────── Results ───────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct FlowResult {
    pub flow_id: FlowId,
    pub src: NodeId,
    pub dst: NodeId,
    pub size_bytes: u64,
    pub start_ns: u64,
    pub end_ns: Option<u64>,
    pub fct_ns: Option<u64>,
    pub total_flits: u32,
    pub delivered: u32,
    pub dropped: u32,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub struct SimResult {
    pub flows: Vec<FlowResult>,
    pub links: HashMap<(NodeId, NodeId), LinkStats>,
    pub total_time_ns: u64,
    pub flit_size_bytes: u32,
}

impl SimResult {
    pub fn success_count(&self) -> usize {
        self.flows.iter().filter(|f| f.success).count()
    }

    pub fn avg_fct_ns(&self) -> Option<f64> {
        let mut sum = 0u64;
        let mut n = 0u64;
        for f in &self.flows {
            if let Some(v) = f.fct_ns {
                sum += v;
                n += 1;
            }
        }
        if n == 0 {
            None
        } else {
            Some(sum as f64 / n as f64)
        }
    }

    pub fn percentile_fct_ns(&self, p: f64) -> Option<u64> {
        let mut fcts: Vec<u64> = self.flows.iter().filter_map(|f| f.fct_ns).collect();
        if fcts.is_empty() {
            return None;
        }
        fcts.sort_unstable();
        let idx = ((p / 100.0) * (fcts.len() as f64 - 1.0)).round() as usize;
        Some(fcts[idx])
    }

    pub fn total_dropped(&self) -> u64 {
        self.links.values().map(|l| l.flits_dropped).sum()
    }
}