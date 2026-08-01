# ADR-001: Language Choice - Rust over C++

## Status
Accepted

## Context

GFCS-R requires implementing a discrete-event network simulator core
(event queue, graph-based topology, per-link mutable state updated
across potentially large numbers of events) plus a congestion-aware
adaptive routing engine. Both C++ and Rust are viable candidates:
C++ has a mature ecosystem for network simulation (e.g. NS-3,
OMNeT++) and is the conventional choice in HPC/networking research
tooling.

The simulator core's central data structures — the event queue and
the topology graph — involve heavily shared, frequently mutated
state accessed across thousands of discrete events per run. This is
the category of code where manual memory management bugs(use-after-free,
dangling pointers, iterator invalidation on graphmutation) 
are both easy to introduce and hard to diagnose, since symptoms 
often surface far from the root cause.

The project is being built as a from-scratch simulator with a custom
adaptive routing algorithm as its primary contribution, rather than
as an extension or plugin to an existing simulation framework.

## Decision

Implement GFCS-R in Rust, using `petgraph` for the topology graph and
a `BinaryHeap`-based min-heap for the event queue.

## Rationale

- The simulator core's shared mutable state (event queue, per-link
  congestion state, topology graph) is the highest-risk area of the
  codebase for memory-safety bugs. Rust's ownership model catches
  use-after-free and dangling-reference errors at compile time,
  removing an entire class of bugs from the riskiest phase of the
  project (the discrete-event core).
- Rust's ownership model maps naturally onto graph and event-queue
  structures via `petgraph` and standard collections, without
  resorting to raw pointers or pervasive `shared_ptr` usage that
  would be needed for equivalent safety guarantees in C++.
- The rest of the author's active systems portfolio (FerumFS, DKVS,
  GraphStream) is written in Rust. A single-language systems stack
  avoids context-switching cost between projects that already share
  a rotating daily schedule, and allows direct reuse of patterns
  (e.g. WAL/event-queue design from FerumFS) across projects.
- C++'s main advantage - the mature NS-3/OMNeT++ ecosystem - only
  applies if the project were built as an extension of an existing
  simulation framework. Since GFCS-R is a from-scratch simulator
  built specifically to give full control over the routing algorithm
  implementation, this advantage does not apply.

## Consequences

- Some development velocity cost is expected in Phase 1-2 (topology
  generator, simulator core), where satisfying the borrow checker on
  graph-heavy structures can require more iteration than the
  equivalent C++ code would.
- No access to NS-3/OMNeT++ tooling or existing network-simulation
  building blocks; all topology generation, event scheduling, and
  link modeling must be implemented from scratch.

