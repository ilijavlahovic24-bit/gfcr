use std::path::PathBuf;
use petgraph::visit::EdgeRef;
use anyhow::Result;
use clap::Parser;
use topology::{NodeKind, TopologyConfig, TopologyGraph};

#[derive(Parser, Debug)]
#[command(name = "gfcs-r", about = "GPU Fabric Congestion Simulator")]
struct Args {
    /// Putanja do YAML topology config-a
    #[arg(short, long)]
    topology: PathBuf,

    /// Ispisi strukturu grafa i izadji
    #[arg(long)]
    dump_graph: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config = TopologyConfig::load_from_file(&args.topology)?;
    let graph = config.build()?;

    if args.dump_graph {
        dump_graph(&graph);
    }
    Ok(())
}

fn dump_graph(g: &TopologyGraph) {
    println!("nodes={} edges={}", g.node_count(), g.edge_count());
    println!("\nNodes:");
    for id in g.inner().node_indices() {
        let node = g.node(id).unwrap();
        let kind = match &node.kind {
            NodeKind::Host { gpu_id }               => format!("host(gpu={gpu_id})"),
            NodeKind::EdgeSwitch { pod, index }     => format!("edge(pod={pod},idx={index})"),
            NodeKind::AggSwitch  { pod, index }     => format!("agg(pod={pod},idx={index})"),
            NodeKind::CoreSwitch { index }          => format!("core(idx={index})"),
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