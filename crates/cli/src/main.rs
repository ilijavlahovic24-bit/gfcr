use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use petgraph::visit::EdgeRef;
use routing::ecmp::EcmpRouter;
use routing::adaptive::AdaptiveRouter; // <-- DODATO
use sim_core::{Router, Simulator};     // <-- DODAT Router
use topology::{NodeKind, TopologyConfig, TopologyGraph};
use traffic_gen::{all_to_all, ring_allreduce};
use metrics::MetricsWriter;

#[derive(Parser, Debug)]
#[command(name = "gfcs-r", about = "GPU Fabric Congestion Simulator")]
struct Args {
    #[arg(short, long)]
    topology: PathBuf,

    #[arg(long)]
    dump_graph: bool,

    #[arg(long)]
    scenario: Option<String>,

    /// Routing algorithm to use: "ecmp" or "adaptive"
    #[arg(long, default_value = "ecmp")]
    router: String,

    #[arg(long, default_value = "output")]
    out_dir: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let config = TopologyConfig::load_from_file(&args.topology)
        .context("Failed to load topology config")?;
    let graph = config.build()
        .context("Failed to build topology graph")?;

    if args.dump_graph {
        dump_graph(&graph);
        return Ok(());
    }

    if let Some(scenario) = args.scenario {
        run_scenario(&scenario, &args.router, graph, &args.out_dir)?;
    } else {
        println!("No scenario provided. Use --scenario to run a simulation.");
    }

    Ok(())
}

fn run_scenario(scenario: &str, router_type: &str, graph: TopologyGraph, out_dir: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;

    let hosts = graph.hosts();
    if hosts.is_empty() {
        anyhow::bail!("Topology has no hosts (GPUs)");
    }

    println!("Running scenario '{}' with '{}' router on {} GPUs...", scenario, router_type, hosts.len());

    let mut flows = match scenario {
        "all-to-all" => all_to_all::generate(hosts.len(), 1024 * 1024), // 1 MB per pair
        "ring-allreduce" => ring_allreduce::generate(hosts.len(), 1024 * 1024), // 1 MB total data
        _ => anyhow::bail!("Unknown scenario: {}", scenario),
    };

    for flow in &mut flows {
        flow.src = hosts[flow.src.index()];
        flow.dst = hosts[flow.dst.index()];
    }

    // Select router dynamically based on CLI argument
    let router: Box<dyn Router> = match router_type {
        "ecmp" => Box::new(EcmpRouter),
        "adaptive" => Box::new(AdaptiveRouter),
        _ => anyhow::bail!("Unknown router: {}. Use 'ecmp' or 'adaptive'.", router_type),
    };

    let result = Simulator::new(graph, flows, router).run();

    let fct_path = out_dir.join("fct.csv");
    let util_path = out_dir.join("utilization.csv");
    let summary_path = out_dir.join("summary.json");

    MetricsWriter::export_fct_csv(&result, &fct_path)?;
    MetricsWriter::export_utilization_csv(&result, &util_path)?;
    MetricsWriter::export_summary_json(&result, &summary_path)?;

    println!("Simulation complete! Metrics exported to {:?}", out_dir);
    Ok(())
}

fn dump_graph(g: &TopologyGraph) {
    println!("nodes={} edges={}", g.node_count(), g.edge_count());
    println!("\nNodes:");
    for id in g.inner().node_indices() {
        let node = g.node(id).unwrap();
        let kind = match &node.kind {
            NodeKind::Host { gpu_id } => format!("host(gpu={gpu_id})"),
            NodeKind::EdgeSwitch { pod, index } => format!("edge(pod={pod},idx={index})"),
            NodeKind::AggSwitch { pod, index } => format!("agg(pod={pod},idx={index})"),
            NodeKind::CoreSwitch { index } => format!("core(idx={index})"),
        };
        println!("  [{:>3}] {kind}", id.index());
    }
    println!("\nLinks:");
    for er in g.inner().edge_references() {
        let a = er.source().index();
        let b = er.target().index();
        let w = er.weight();
        println!(
            "  [{a:>3}] <-> [{b:>3}]  bw={:.1}Gbps lat={}ns buf={}",
            w.bandwidth_gbps, w.latency_ns, w.buffer_size
        );
    }
}