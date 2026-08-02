# GFCS-R — GPU Fabric Congestion Simulation & Router

Discrete-event simulator for GPU interconnect fabrics (fat-tree topologies) with a custom congestion-aware adaptive routing engine, built to model and reduce tail latency under the traffic patterns typical of distributed ML training (all-reduce, all-to-all/MoE, pipeline parallelism).

**Status:** Planned

## Overview

Communication patterns dominate wall-clock time in large-scale distributed training, and static routing (ECMP) degrades badly under bursty "elephant flow" traffic such as gradient all-reduce or MoE expert dispatch. GFCS-R simulates a GPU interconnect fabric end-to-end — topology, traffic, and routing — to measure that degradation and demonstrate improvement through congestion-aware adaptive routing.

## Architecture

```
Topology Generator → Discrete-Event Simulator Core → Routing Engine
                              ↓
                     Traffic Generator (workload models)
                              ↓
                   Metrics & Visualization Layer
```

- **Topology Generator** — k-ary fat-tree construction via `petgraph`
- **Simulator Core** — event-driven (not time-stepped), flow-level fidelity, max-min fair link bandwidth model
- **Traffic Generator** — ring/tree all-reduce, all-to-all (MoE), pipeline parallel workload models, validated against theoretical FCT formulas
- **Routing Engine** — ECMP and Valiant baselines vs. a custom congestion-aware adaptive router (piggybacked link-state feedback, inspired by Conga/Hula)
- **Metrics** — Flow Completion Time distributions (p50/p95/p99), per-link utilization heatmaps, congestion hotspot detection

## Tech Stack

| Layer | Technology |
|---|---|
| Core | Rust |
| Graph | `petgraph` |
| Config | `serde` + YAML |
| CLI | `clap` |
| Testing | built-in `#[test]` + `proptest` |
| Visualization | Python (matplotlib/plotly), decoupled from the simulator core |

## Project Structure

```
gfcs-r/
├── crates/
│   ├── topology/     # topology generation
│   ├── sim-core/     # discrete-event simulator core
│   ├── traffic-gen/  # ML workload traffic models
│   ├── routing/       # ECMP, Valiant, adaptive congestion-aware router
│   ├── metrics/       # FCT/utilization tracking and export
│   └── cli/           # orchestration binary
├── configs/           # topology and traffic YAML configs
├── viz/                # Python visualization scripts
└── docs/adr/           # architecture decision records
```

## Development Plan

Full phase-by-phase breakdown (topology → simulator core → traffic generator → ECMP baseline → adaptive router → metrics/visualization → polish) is tracked in `docs/DEVELOPMENT_PLAN.md`, along with per-phase definitions of done.

## Architecture Decision Records

Key design decisions are documented as ADRs in `docs/adr/`, including:
- **ADR-001** — Language choice: Rust over C++

## Relation to Other Projects

GFCS-R is designed to optionally consume traffic traces from [FerumFS](#) (dataset shard reads, checkpoint write bursts) as an additional workload generator, connecting the storage and network layers of a distributed training pipeline — via a decoupled trace-file interface, not a code dependency.

## Roadmap

- [ ] Phase 1 — Topology generator
- [ ] Phase 2 — Discrete-event simulator core
- [ ] Phase 3 — Traffic generator
- [ ] Phase 4 — ECMP baseline routing
- [ ] Phase 5 — Adaptive congestion-aware router
- [ ] Phase 6 — Metrics & visualization
- [ ] Phase 7 — Testing, comparison, polish
- [ ] Phase 8 (stretch) — Flit-level fidelity, dragonfly topology, FerumFS trace integration
