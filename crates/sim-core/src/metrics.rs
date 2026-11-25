//! Agregacija `SimResult` → `MetricsReport`.

use std::collections::HashMap;

use petgraph::graph::NodeIndex;
use serde::Serialize;

use crate::flit_engine::{FlowResult, SimResult};
use topology::graph::LinkClass;
use crate::TopologyGraph;

type NodeId = NodeIndex;

// ─────────── Tipovi ───────────

#[derive(Debug, Clone, Serialize)]
pub struct SummaryMetrics {
    pub total_flows: usize,
    pub successful_flows: usize,
    pub failed_flows: usize,
    pub total_bytes: u64,
    pub delivered_bytes: u64,
    pub total_flits: u64,
    pub delivered_flits: u64,
    pub dropped_flits: u64,
    pub avg_fct_ns: Option<f64>,
    pub p50_fct_ns: Option<u64>,
    pub p90_fct_ns: Option<u64>,
    pub p99_fct_ns: Option<u64>,
    pub p999_fct_ns: Option<u64>,
    pub max_fct_ns: Option<u64>,
    pub total_time_ns: u64,
    pub goodput_gbps: f64,
    pub avg_hops: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassMetrics {
    pub class_name: String,
    pub link_count: usize,
    pub flits_forwarded: u64,
    pub flits_dropped: u64,
    pub bytes_forwarded: u64,
    pub avg_utilization: f64,
    pub max_utilization: f64,
    pub avg_queue_depth: f64,
    pub max_queue_depth: usize,
    pub total_busy_ns: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkReport {
    pub from: usize,
    pub to: usize,
    pub class: String,
    pub utilization: f64,
    pub avg_queue_depth: f64,
    pub max_queue_depth: usize,
    pub flits_forwarded: u64,
    pub flits_dropped: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FctBucket {
    pub start_ns: u64,
    pub end_ns: u64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncastEvent {
    pub dst: usize,
    pub max_concurrent_flows: usize,
    pub peak_ns: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsReport {
    pub summary: SummaryMetrics,
    pub per_class: Vec<ClassMetrics>,
    pub per_link: Vec<LinkReport>,
    pub fct_histogram: Vec<FctBucket>,
    pub top_slowest_flows: Vec<FlowResult>,
    pub incast_events: Vec<IncastEvent>,
}

// ─────────── Helperi ───────────

fn class_of(topology: &TopologyGraph, u: NodeId, v: NodeId) -> String {
    match topology.edge_between(u, v) {
        Some(link) => match link.class {
            LinkClass::Generic  => "Generic".into(),
            LinkClass::Terminal => "Terminal".into(),
            LinkClass::Local    => "Local".into(),
            LinkClass::Global   => "Global".into(),
        },
        None => "Unknown".into(),
    }
}

fn percentile(sorted: &[u64], p: f64) -> Option<u64> {
    if sorted.is_empty() { return None; }
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    Some(sorted[idx.min(sorted.len() - 1)])
}

fn histogram(values: &[u64], buckets: usize) -> Vec<FctBucket> {
    if values.is_empty() { return vec![]; }
    let min = *values.iter().min().unwrap();
    let max = *values.iter().max().unwrap();
    if min == max {
        return vec![FctBucket { start_ns: min, end_ns: max, count: values.len() }];
    }
    let bucket_size = ((max - min) as f64 / buckets as f64).ceil() as u64;
    let bucket_size = bucket_size.max(1);
    let mut counts = vec![0usize; buckets];
    for &v in values {
        let idx = ((v - min) / bucket_size) as usize;
        counts[idx.min(buckets - 1)] += 1;
    }
    (0..buckets).map(|i| {
        let start = min + i as u64 * bucket_size;
        FctBucket { start_ns: start, end_ns: start + bucket_size, count: counts[i] }
    }).collect()
}

// ─────────── Glavna funkcija ───────────

pub fn build_report(
    result: &SimResult,
    topology: &TopologyGraph,
    top_n_flows: usize,
) -> MetricsReport {
    let total_time_ns = result.total_time_ns.max(1);

    // ─── Summary ───
    let mut fcts: Vec<u64> = Vec::with_capacity(result.flows.len());
    let mut total_bytes = 0u64;
    let mut delivered_bytes = 0u64;
    let mut total_flits = 0u64;
    let mut delivered_flits = 0u64;
    let mut dropped_flits = 0u64;
    let mut successful = 0usize;

    for f in &result.flows {
        total_bytes += f.size_bytes;
        total_flits += f.total_flits as u64;
        delivered_flits += f.delivered as u64;
        dropped_flits += f.dropped as u64;
        if let Some(fct) = f.fct_ns {
            fcts.push(fct);
            if f.success {
                delivered_bytes += f.size_bytes;
                successful += 1;
            }
        }
    }
    fcts.sort_unstable();

    let goodput_gbps = (delivered_bytes as f64 * 8.0) / total_time_ns as f64;

    let summary = SummaryMetrics {
        total_flows: result.flows.len(),
        successful_flows: successful,
        failed_flows: result.flows.len() - successful,
        total_bytes,
        delivered_bytes,
        total_flits,
        delivered_flits,
        dropped_flits,
        avg_fct_ns: if fcts.is_empty() { None }
        else { Some(fcts.iter().sum::<u64>() as f64 / fcts.len() as f64) },
        p50_fct_ns:  percentile(&fcts, 50.0),
        p90_fct_ns:  percentile(&fcts, 90.0),
        p99_fct_ns:  percentile(&fcts, 99.0),
        p999_fct_ns: percentile(&fcts, 99.9),
        max_fct_ns:  fcts.last().copied(),
        total_time_ns,
        goodput_gbps,
        avg_hops: None, // popuni ako imaš putanje u rezultatu
    };

    // ─── Per-class + per-link ───
    let mut class_accum: HashMap<String, ClassAccum> = HashMap::new();
    let mut per_link: Vec<LinkReport> = Vec::with_capacity(result.links.len());

    for ((u, v), stats) in &result.links {
        let class_name = class_of(topology, *u, *v);
        let util = stats.utilization(total_time_ns);

        per_link.push(LinkReport {
            from: u.index(),
            to: v.index(),
            class: class_name.clone(),
            utilization: util,
            avg_queue_depth: stats.avg_queue_depth(),
            max_queue_depth: stats.max_queue_depth,
            flits_forwarded: stats.flits_forwarded,
            flits_dropped: stats.flits_dropped,
        });

        let acc = class_accum.entry(class_name.clone()).or_insert_with(|| ClassAccum {
            class_name,
            ..Default::default()
        });
        acc.link_count += 1;
        acc.flits_forwarded += stats.flits_forwarded;
        acc.flits_dropped += stats.flits_dropped;
        acc.bytes_forwarded += stats.bytes_forwarded;
        acc.total_busy_ns += stats.total_busy_ns;
        acc.max_utilization = acc.max_utilization.max(util);
        acc.max_queue_depth = acc.max_queue_depth.max(stats.max_queue_depth);
        acc.q_sum += stats.queue_depth_sum;
        acc.q_samples += stats.queue_depth_samples;
    }

    per_link.sort_by(|a, b| b.utilization.partial_cmp(&a.utilization).unwrap_or(std::cmp::Ordering::Equal));

    let mut per_class: Vec<ClassMetrics> = class_accum.into_values().map(|a| {
        ClassMetrics {
            class_name: a.class_name,
            link_count: a.link_count,
            flits_forwarded: a.flits_forwarded,
            flits_dropped: a.flits_dropped,
            bytes_forwarded: a.bytes_forwarded,
            avg_utilization: if a.link_count > 0 && total_time_ns > 0 {
                a.total_busy_ns as f64 / (total_time_ns as f64 * a.link_count as f64)
            } else { 0.0 },
            max_utilization: a.max_utilization,
            avg_queue_depth: if a.q_samples > 0 {
                a.q_sum as f64 / a.q_samples as f64
            } else { 0.0 },
            max_queue_depth: a.max_queue_depth,
            total_busy_ns: a.total_busy_ns,
        }
    }).collect();
    per_class.sort_by(|a, b| a.class_name.cmp(&b.class_name));

    // ─── FCT histogram ───
    let fct_histogram = histogram(&fcts, 20);

    // ─── Top N najsporijih flow-ova ───
    let mut flows_with_fct: Vec<FlowResult> =
        result.flows.iter().filter(|f| f.fct_ns.is_some()).cloned().collect();
    flows_with_fct.sort_by(|a, b| b.fct_ns.cmp(&a.fct_ns));
    flows_with_fct.truncate(top_n_flows);

    // ─── Incast ───
    let incast_events = detect_incast(&result.flows, 4);

    MetricsReport {
        summary,
        per_class,
        per_link,
        fct_histogram,
        top_slowest_flows: flows_with_fct,
        incast_events,
    }
}

// ─────────── Incast detekcija ───────────

fn detect_incast(flows: &[FlowResult], min_concurrent: usize) -> Vec<IncastEvent> {
    // Grupiši po dst, uzmi (start, end) interval
    let mut by_dst: HashMap<usize, Vec<(u64, u64)>> = HashMap::new();
    for f in flows {
        if let (Some(end), Some(_)) = (f.end_ns, f.fct_ns) {
            by_dst.entry(f.dst.index()).or_default().push((f.start_ns, end));
        }
    }

    let mut out = Vec::new();
    for (dst, intervals) in by_dst {
        // Sweep line
        let mut events: Vec<(u64, i32)> = Vec::with_capacity(intervals.len() * 2);
        for (s, e) in &intervals {
            events.push((*s, 1_i32));
            events.push((*e, -1_i32));
        }
        events.sort_by_key(|(t, d)| (*t, *d));

        let mut cur = 0i32;
        let mut max = 0i32;
        let mut peak = 0u64;
        for (t, d) in events {
            cur += d;
            if cur > max { max = cur; peak = t; }
        }
        if (max as usize) >= min_concurrent {
            out.push(IncastEvent {
                dst,
                max_concurrent_flows: max as usize,
                peak_ns: peak,
            });
        }
    }
    out.sort_by(|a, b| b.max_concurrent_flows.cmp(&a.max_concurrent_flows));
    out
}

// ─────────── Interni akumulator ───────────

#[derive(Default)]
struct ClassAccum {
    class_name: String,
    link_count: usize,
    flits_forwarded: u64,
    flits_dropped: u64,
    bytes_forwarded: u64,
    total_busy_ns: u64,
    max_utilization: f64,
    max_queue_depth: usize,
    q_sum: u64,
    q_samples: u64,
}