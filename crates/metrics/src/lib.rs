use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use sim_core::{LinkUtilizationRecord, SimulationResult};

pub struct MetricsWriter;

impl MetricsWriter {
    /// Exports per-flow metrics to CSV.
    pub fn export_fct_csv(result: &SimulationResult, path: &Path) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writeln!(writer, "flow_id,src,dst,size_bytes,start_time_ns,end_time_ns,fct_ns")?;
        for f in &result.completed_flows {
            writeln!(
                writer,
                "{},{},{},{},{},{},{}",
                f.id, f.src.index(), f.dst.index(), f.size_bytes, f.start_time_ns, f.end_time_ns, f.fct_ns
            )?;
        }
        Ok(())
    }

    /// Exports link utilization history to CSV.
    pub fn export_utilization_csv(result: &SimulationResult, path: &Path) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writeln!(writer, "time_ns,link_id,utilization_pct")?;
        for r in &result.utilization_history {
            writeln!(writer, "{},{},{:.2}", r.time_ns, r.link_id, r.utilization_pct)?;
        }
        Ok(())
    }

    /// Exports a JSON summary with percentiles.
    pub fn export_summary_json(result: &SimulationResult, path: &Path) -> std::io::Result<()> {
        let mut fcts: Vec<u64> = result.completed_flows.iter().map(|f| f.fct_ns).collect();
        fcts.sort_unstable();

        let p50 = percentile(&fcts, 0.50);
        let p95 = percentile(&fcts, 0.95);
        let p99 = percentile(&fcts, 0.99);

        let max_utilization = result.utilization_history
            .iter()
            .map(|r| r.utilization_pct)
            .fold(0.0_f64, f64::max);

        let congested_links = result.utilization_history
            .iter()
            .filter(|r| r.utilization_pct > 90.0)
            .map(|r| r.link_id)
            .collect::<std::collections::HashSet<_>>()
            .len();

        let summary = serde_json::json!({
            "total_flows": result.completed_flows.len(),
            "sim_time_ns": result.sim_time_ns,
            "fct_p50_ns": p50,
            "fct_p95_ns": p95,
            "fct_p99_ns": p99,
            "max_utilization_pct": max_utilization,
            "congested_links_count": congested_links,
        });

        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &summary)?;
        Ok(())
    }
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() { return 0; }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx]
}