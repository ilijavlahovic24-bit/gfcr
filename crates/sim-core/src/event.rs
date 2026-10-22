//! Simulation events and the event queue.
//!
//! Events are ordered by `(time_ns, seq)`. The `seq` field is a monotonically
//! increasing counter that breaks ties: events at the same simulation time
//! fire in insertion order (FIFO). The queue is a min-heap implemented as
//! `BinaryHeap<Reverse<Event>>` — no external event-loop crate is needed
//! because MVP is single-threaded.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::flow::{Flow, FlowId};

#[derive(Debug)]
pub enum EventKind {
    /// A flow enters the network. Carries the full flow description so the
    /// simulator does not need a separate lookup table.
    FlowArrival { flow: Flow },

    /// A flow's last bit reaches its destination.
    ///
    /// `expected_ns` is the delivery time at the moment the event was
    /// scheduled. If `flow.expected_complete_ns` no longer matches this
    /// value, the event is stale (a later event supersedes it) and is
    /// ignored. This lets us avoid removing events from the heap.
    FlowComplete { flow_id: FlowId, expected_ns: u64 },
}

#[derive(Debug)]
pub struct Event {
    pub time_ns: u64,
    pub seq: u64,
    pub kind: EventKind,
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.time_ns == other.time_ns && self.seq == other.seq
    }
}
impl Eq for Event {}
impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Event {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time_ns
            .cmp(&other.time_ns)
            .then_with(|| self.seq.cmp(&other.seq))
    }
}

pub struct EventQueue {
    heap: BinaryHeap<Reverse<Event>>,
    next_seq: u64,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            next_seq: 0,
        }
    }

    pub fn push(&mut self, time_ns: u64, kind: EventKind) {
        let event = Event {
            time_ns,
            seq: self.next_seq,
            kind,
        };
        self.next_seq += 1;
        self.heap.push(Reverse(event));
    }

    pub fn pop(&mut self) -> Option<Event> {
        self.heap.pop().map(|Reverse(e)| e)
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_flow() -> Flow {
        Flow {
            id: 0,
            src: petgraph::graph::NodeIndex::new(0),
            dst: petgraph::graph::NodeIndex::new(1),
            size_bytes: 1,
            start_time_ns: 0,
        }
    }

    #[test]
    fn pops_in_time_order() {
        let mut q = EventQueue::new();
        q.push(100, EventKind::FlowComplete { flow_id: 0, expected_ns: 100 });
        q.push(50, EventKind::FlowComplete { flow_id: 1, expected_ns: 50 });
        q.push(200, EventKind::FlowComplete { flow_id: 2, expected_ns: 200 });
        assert_eq!(q.pop().unwrap().time_ns, 50);
        assert_eq!(q.pop().unwrap().time_ns, 100);
        assert_eq!(q.pop().unwrap().time_ns, 200);
        assert!(q.pop().is_none());
    }

    #[test]
    fn fifo_breaks_ties() {
        let mut q = EventQueue::new();
        q.push(10, EventKind::FlowArrival { flow: dummy_flow() });
        // same timestamp, different kind — inserted second, pops second
        q.push(10, EventKind::FlowComplete { flow_id: 0, expected_ns: 10 });
        let first = q.pop().unwrap();
        let second = q.pop().unwrap();
        assert_eq!(first.seq + 1, second.seq);
        assert!(matches!(first.kind, EventKind::FlowArrival { .. }));
    }
}