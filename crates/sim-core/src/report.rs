//! Ispis `MetricsReport`-a: ljudski čitljivo, CSV, JSON.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use crate::metrics::MetricsReport;

// ─────────── Ispis ───────────

pub fn print_report(r: &MetricsReport) -> io::Result<()> {
    let s = &r.summary;
    println!("═══ SUMMARY ═══");
    println!("flows:       {} (ok {}, fail {})", s.total_flows, s.successful_flows, s.failed_flows);
    println!("bytes:       {} / {} delivered", s.delivered_bytes, s.total_bytes);
    println!("flits:       {} / {} delivered ({} dropped)",
             s.delivered_flits, s.total_flits, s.dropped_flits);
    println!("goodput:     {:.2} Gbps", s.goodput_gbps);
    println!("time:        {:.3} ms", s.total_time_ns as f64 / 1e6);
    println!();
    println!("FCT (ns):");
    if let Some(v) = s.avg_fct_ns  { println!("  avg  {:.0}", v); }
    if let Some(v) = s.p50_fct_ns  { println!("  p50  {}", v); }
    if let Some(v) = s.p90_fct_ns  { println!("  p90  {}", v); }
    if let Some(v) = s.p99_fct_ns  { println!("  p99  {}", v); }
    if let Some(v) = s.p999_fct_ns { println!("  p999 {}", v); }
    if let Some(v) = s.max_fct_ns  { println!("  max  {}", v); }

    println!();
    println!("═══ PER CLASS ═══");
    println!("{:<10} {:>5} {:>10} {:>10} {:>8} {:>8} {:>10}",
             "class", "links", "fwd", "dropped", "avgU%", "maxU%", "maxQ");
    for c in &r.per_class {
        println!("{:<10} {:>5} {:>10} {:>10} {:>8.2} {:>8.2} {:>10}",
                 c.class_name, c.link_count,
                 c.flits_forwarded, c.flits_dropped,
                 c.avg_utilization * 100.0, c.max_utilization * 100.0,
                 c.max_queue_depth);
    }

    println!();
    println!("═══ TOP {} SLOWEST FLOWS ═══", r.top_slowest_flows.len());
    println!("{:<6} {:<6} {:<6} {:>10} {:>14}",
             "flow", "src", "dst", "size", "fct_ns");
    for f in &r.top_slowest_flows {
        println!("{:<6} {:<6} {:<6} {:>10} {:>14}",
                 f.flow_id, f.src.index(), f.dst.index(),
                 f.size_bytes, f.fct_ns.unwrap_or(0));
    }

    if !r.incast_events.is_empty() {
        println!();
        println!("═══ INCAST (>=4 concurrent) ═══");
        for e in &r.incast_events {
            println!("dst {:>4}: max {} concurrent @ {:.3} ms",
                     e.dst, e.max_concurrent_flows,
                     e.peak_ns as f64 / 1e6);
        }
    }
    Ok(())
}

// ─────────── CSV ───────────

pub fn write_csv_summary(r: &MetricsReport, path: &Path) -> io::Result<()> {
    let mut f = File::create(path)?;
    let s = &r.summary;
    writeln!(f, "metric,value")?;
    writeln!(f, "total_flows,{}", s.total_flows)?;
    writeln!(f, "successful_flows,{}", s.successful_flows)?;
    writeln!(f, "failed_flows,{}", s.failed_flows)?;
    writeln!(f, "total_bytes,{}", s.total_bytes)?;
    writeln!(f, "delivered_bytes,{}", s.delivered_bytes)?;
    writeln!(f, "total_flits,{}", s.total_flits)?;
    writeln!(f, "delivered_flits,{}", s.delivered_flits)?;
    writeln!(f, "dropped_flits,{}", s.dropped_flits)?;
    writeln!(f, "avg_fct_ns,{}", s.avg_fct_ns.unwrap_or(0.0))?;
    writeln!(f, "p50_fct_ns,{}", s.p50_fct_ns.unwrap_or(0))?;
    writeln!(f, "p90_fct_ns,{}", s.p90_fct_ns.unwrap_or(0))?;
    writeln!(f, "p99_fct_ns,{}", s.p99_fct_ns.unwrap_or(0))?;
    writeln!(f, "p999_fct_ns,{}", s.p999_fct_ns.unwrap_or(0))?;
    writeln!(f, "max_fct_ns,{}", s.max_fct_ns.unwrap_or(0))?;
    writeln!(f, "total_time_ns,{}", s.total_time_ns)?;
    writeln!(f, "goodput_gbps,{:.6}", s.goodput_gbps)?;
    Ok(())
}

pub fn write_csv_per_class(r: &MetricsReport, path: &Path) -> io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "class,links,flits_forwarded,flits_dropped,bytes_forwarded,avg_util,max_util,avg_qdepth,max_qdepth")?;
    for c in &r.per_class {
        writeln!(f,
                 "{},{},{},{},{},{:.6},{:.6},{:.6},{}",
                 c.class_name, c.link_count,
                 c.flits_forwarded, c.flits_dropped, c.bytes_forwarded,
                 c.avg_utilization, c.max_utilization,
                 c.avg_queue_depth, c.max_queue_depth)?;
    }
    Ok(())
}

pub fn write_csv_per_link(r: &MetricsReport, path: &Path) -> io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "from,to,class,utilization,avg_qdepth,max_qdepth,flits_forwarded,flits_dropped")?;
    for l in &r.per_link {
        writeln!(f, "{},{},{},{:.6},{:.6},{},{},{}",
                 l.from, l.to, l.class, l.utilization,
                 l.avg_queue_depth, l.max_queue_depth,
                 l.flits_forwarded, l.flits_dropped)?;
    }
    Ok(())
}

pub fn write_csv_fct_histogram(r: &MetricsReport, path: &Path) -> io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(f, "bucket_start_ns,bucket_end_ns,count")?;
    for b in &r.fct_histogram {
        writeln!(f, "{},{},{}", b.start_ns, b.end_ns, b.count)?;
    }
    Ok(())
}

// ─────────── JSON ───────────

pub fn write_json(r: &MetricsReport, path: &Path) -> io::Result<()> {
    let f = File::create(path)?;
    serde_json::to_writer_pretty(f, r)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}