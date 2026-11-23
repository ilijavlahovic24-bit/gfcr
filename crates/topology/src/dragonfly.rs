//! Dragonfly topology generator (Kim et al., 2008).
//!
//! Node layout in `TopologyGraph` (deterministic, because `add_node` assigns indices in order):
//! hosts : [0, g*a*p)
//! routers : [g*a*p, g*a*p + g*a)
//!
//! Link classes: Terminal / Local / Global (see `LinkClass`).

use petgraph::graph::NodeIndex;

use crate::graph::{Link, LinkClass, NodeKind, TopologyGraph};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DragonflyParams {
    pub a: usize,
    pub p: usize,
    pub h: usize,
    pub g: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct LinkParams {
    pub terminal_bw: f64,
    pub terminal_lat: u64,
    pub terminal_buf: usize,
    pub local_bw: f64,
    pub local_lat: u64,
    pub local_buf: usize,
    pub global_bw: f64,
    pub global_lat: u64,
    pub global_buf: usize,
}

impl Default for LinkParams {
    fn default() -> Self {
        Self {
            terminal_bw: 200.0, terminal_lat: 100, terminal_buf: 64,
            local_bw:    200.0, local_lat:    100, local_buf:    64,
            global_bw:   100.0, global_lat:   500, global_buf:   128,
        }
    }
}

impl DragonflyParams {
    #[inline] pub fn host_count(&self)   -> usize { self.g * self.a * self.p }
    #[inline] pub fn router_count(&self) -> usize { self.g * self.a }
    #[inline] pub fn node_count(&self)   -> usize { self.host_count() + self.router_count() }

    #[inline]
    pub fn host_id(&self, group: usize, router: usize, host: usize) -> usize {
        debug_assert!(group < self.g && router < self.a && host < self.p);
        (group * self.a + router) * self.p + host
    }

    #[inline]
    pub fn router_id(&self, group: usize, router: usize) -> usize {
        debug_assert!(group < self.g && router < self.a);
        self.host_count() + group * self.a + router
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.a < 2 { return Err(format!("dragonfly: a >= 2 (a={})", self.a)); }
        if self.p < 1 { return Err(format!("dragonfly: p >= 1 (p={})", self.p)); }
        if self.h < 1 || self.h > self.a {
            return Err(format!("dragonfly: h in [1, a] (h={}, a={})", self.h, self.a));
        }
        if self.g < 2 { return Err(format!("dragonfly: g >= 2 (g={})", self.g)); }
        Ok(())
    }

    pub fn is_perfect(&self) -> bool { self.g == self.a * self.h + 1 }
}

pub fn generate(params: DragonflyParams, lp: LinkParams) -> Result<TopologyGraph, String> {
    params.validate()?;
    let DragonflyParams { a, p, h, g } = params;

    let mut t = TopologyGraph::new();

    // 1) Hosts — in order (g, r, k) => ID = (g*a + r)*p + k
    for grp in 0..g {
        for r in 0..a {
            for k in 0..p {
                let gpu_id = params.host_id(grp, r, k);
                let _ = t.add_node(NodeKind::Host { gpu_id });
            }
        }
    }

    // 2) Routers — in order (g, r) => ID = host_count + g*a + r
    for grp in 0..g {
        for r in 0..a {
            let _ = t.add_node(NodeKind::DragonflyRouter { group: grp, index: r });
        }
    }

    // petgraph assigns indexes in order, so we can use NodeIndex::new(id).
    let hnode = |id: usize| NodeIndex::new(id);

    // 3) Terminal links (host <-> router)
    for grp in 0..g {
        for r in 0..a {
            let rid = params.router_id(grp, r);
            for k in 0..p {
                let hid = params.host_id(grp, r, k);
                t.add_link(hnode(hid), hnode(rid), Link {
                    bandwidth_gbps: lp.terminal_bw,
                    latency_ns: lp.terminal_lat,
                    buffer_size: lp.terminal_buf,
                    class: LinkClass::Terminal,
                });
            }
        }
    }

    // 4) Local links (full mesh within the group)
    for grp in 0..g {
        for r1 in 0..a {
            for r2 in (r1 + 1)..a {
                let id1 = params.router_id(grp, r1);
                let id2 = params.router_id(grp, r2);
                t.add_link(hnode(id1), hnode(id2), Link {
                    bandwidth_gbps: lp.local_bw,
                    latency_ns: lp.local_lat,
                    buffer_size: lp.local_buf,
                    class: LinkClass::Local,
                });
            }
        }
    }

    // 5) Global links.
    // Scheme: router (i, r) -> group (i + r*h + k) mod g, router r, for k=1..=h.
    // Dedup via (min_id, max_id): each pair of groups exactly once (perfect case).
    let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    for i in 0..g {
        for r in 0..a {
            for k in 1..=h {
                let j = (i + r * h + k) % g;
                if j == i { continue; }
                let id1 = params.router_id(i, r);
                let id2 = params.router_id(j, r);
                let key = if id1 < id2 { (id1, id2) } else { (id2, id1) };
                if seen.insert(key) {
                    t.add_link(hnode(id1), hnode(id2), Link {
                        bandwidth_gbps: lp.global_bw,
                        latency_ns: lp.global_lat,
                        buffer_size: lp.global_buf,
                        class: LinkClass::Global,
                    });
                }
            }
        }
    }

    Ok(t)
}