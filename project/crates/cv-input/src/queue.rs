//! Priority input queue with emergency stop.
//!
//! Events are ordered by priority (P0 = highest) and FIFO within the same
//! priority level.  The emergency-stop flag drains the queue and prevents
//! new events from being processed.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tracing::{debug, info, trace, warn};

use cv_shared::types::{InputEvent, Priority};

// ---------------------------------------------------------------------------
// QueuedEvent – wrapper that implements Ord for the BinaryHeap
// ---------------------------------------------------------------------------

/// Internal wrapper around [`InputEvent`] that can be stored in a
/// [`BinaryHeap`].
///
/// Ordering is `(priority ASC, sequence ASC)` so the heap pops the
/// highest-priority / oldest event first.
#[derive(Debug, Clone)]
struct QueuedEvent {
    /// `Reverse<u8>` so that *lower* numeric values (P0=0) pop first.
    priority: Reverse<u8>,
    /// Monotonically increasing counter for FIFO within the same priority.
    sequence: u64,
    /// The actual event payload.
    event: InputEvent,
}

impl PartialEq for QueuedEvent {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.sequence == other.sequence
    }
}

impl Eq for QueuedEvent {}

impl PartialOrd for QueuedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // BinaryHeap is a max-heap; we want the *smallest* priority value
        // first, therefore we wrap it in `Reverse`.  Within the same
        // priority the smaller sequence number wins.
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.sequence.cmp(&other.sequence))
    }
}

// ---------------------------------------------------------------------------
// PriorityInputQueue
// ---------------------------------------------------------------------------

/// Thread-safe priority queue for [`InputEvent`]s with emergency-stop support.
///
/// # Priority levels
/// | Level | Enum variant       | Meaning              |
/// |-------|--------------------|----------------------|
/// | P0    | `P0_Emergency`     | Emergency stop       |
/// | P1    | `P1_Human`         | Human input          |
/// | P2    | `P2_AI_Confirmed`  | AI with confirmation |
/// | P3    | `P3_AI_Autonomous` | AI autonomous        |
///
/// # Example
/// ```
/// use cv_input::queue::PriorityInputQueue;
/// use cv_shared::types::*;
///
/// let queue = PriorityInputQueue::new();
///
/// // Push a human input event (P1)
/// let ev = InputEvent::new(
///     EventSource::Human,
///     EventType::MouseMove { x: 10, y: 20 },
///     Priority::P1_Human,
///     1,
/// );
/// queue.push(ev);
///
/// assert!(!queue.is_stopped());
/// let popped = queue.pop();
/// assert!(popped.is_some());
/// ```
#[derive(Debug)]
pub struct PriorityInputQueue {
    /// The underlying heap is protected by a `Mutex` so that `push` / `pop`
    /// can be called from multiple threads.
    heap: Mutex<BinaryHeap<QueuedEvent>>,
    /// Global emergency-stop flag.
    stopped: AtomicBool,
    /// Monotonically increasing sequence counter for FIFO ordering.
    next_seq: Mutex<u64>,
}

impl PriorityInputQueue {
    /// Create an empty `PriorityInputQueue`.
    pub fn new() -> Self {
        Self {
            heap: Mutex::new(BinaryHeap::new()),
            stopped: AtomicBool::new(false),
            next_seq: Mutex::new(0),
        }
    }

    /// Push an [`InputEvent`] into the queue.
    ///
    /// The event's [`Priority`] is translated to a numeric value
    /// (`P0=0 … P3=3`).  If the emergency-stop flag is set the event is
    /// silently dropped and a warning is logged.
    pub fn push(&self, event: InputEvent) {
        if self.stopped.load(Ordering::SeqCst) {
            warn!(?event, "Queue stopped – dropping event");
            return;
        }

        let priority_val = priority_to_u8(&event.priority);
        let seq = {
            let mut seq_lock = self.next_seq.lock().unwrap();
            let s = *seq_lock;
            *seq_lock += 1;
            s
        };

        trace!(priority = priority_val, seq, "push");

        let qev = QueuedEvent {
            priority: Reverse(priority_val),
            sequence: seq,
            event,
        };

        let mut heap = self.heap.lock().unwrap();
        heap.push(qev);
    }

    /// Pop the highest-priority event from the queue.
    ///
    /// Returns `None` when the queue is empty or when the emergency-stop
    /// flag is active.
    pub fn pop(&self) -> Option<InputEvent> {
        if self.stopped.load(Ordering::SeqCst) {
            return None;
        }

        let mut heap = self.heap.lock().unwrap();
        heap.pop().map(|qev| qev.event)
    }

    /// Activate the emergency stop.
    ///
    /// 1. Sets the `stopped` flag to `true`.
    /// 2. **Drains** all pending events from the queue (they are lost).
    ///
    /// After calling this method every subsequent [`push`](Self::push) is a
    /// no-op and [`pop`](Self::pop) always returns `None`.
    pub fn emergency_stop(&self) {
        info!("EMERGENCY STOP activated");

        // Set flag first so concurrent pushes start dropping.
        self.stopped.store(true, Ordering::SeqCst);

        // Drain the heap.
        let mut heap = self.heap.lock().unwrap();
        let dropped = heap.len();
        heap.clear();

        if dropped > 0 {
            warn!(dropped, "Emergency stop drained pending events");
        }
    }

    /// Check whether the emergency stop is active.
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    /// Return the number of events currently in the queue.
    ///
    /// Primarily useful for tests and metrics.
    pub fn len(&self) -> usize {
        let heap = self.heap.lock().unwrap();
        heap.len()
    }

    /// Returns `true` if the queue contains no events.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reset the emergency-stop flag and clear the queue.
    ///
    /// **Use with care** – intended for integration tests and controlled
    /// recovery scenarios only.
    #[cfg(test)]
    pub fn reset(&self) {
        let mut heap = self.heap.lock().unwrap();
        heap.clear();
        self.stopped.store(false, Ordering::SeqCst);
        let mut seq = self.next_seq.lock().unwrap();
        *seq = 0;
        debug!("Queue reset");
    }
}

impl Default for PriorityInputQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a [`Priority`] to its numeric representation.
const fn priority_to_u8(p: &Priority) -> u8 {
    match p {
        Priority::P0_Emergency => 0,
        Priority::P1_Human => 1,
        Priority::P2_AI_Confirmed => 2,
        Priority::P3_AI_Autonomous => 3,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cv_shared::types::{EventSource, EventType, InputEvent, MouseButton, Priority};

    /// Helper: build an `InputEvent` with a given priority and sequence.
    fn make_event(source: EventSource, priority: Priority, seq: u64) -> InputEvent {
        InputEvent::new(
            source,
            EventType::MouseMove { x: 0, y: 0 },
            priority,
            seq,
        )
    }

    // ---- Priority ordering ----

    #[test]
    fn p0_before_p1_before_p2_before_p3() {
        let queue = PriorityInputQueue::new();

        // Push in reverse priority order.
        queue.push(make_event(EventSource::AI, Priority::P3_AI_Autonomous, 1));
        queue.push(make_event(EventSource::AI, Priority::P1_Human, 2));
        queue.push(make_event(EventSource::System, Priority::P0_Emergency, 3));
        queue.push(make_event(EventSource::AI, Priority::P2_AI_Confirmed, 4));

        // Should pop P0 first.
        let first = queue.pop().unwrap();
        assert_eq!(first.priority, Priority::P0_Emergency);

        let second = queue.pop().unwrap();
        assert_eq!(second.priority, Priority::P1_Human);

        let third = queue.pop().unwrap();
        assert_eq!(third.priority, Priority::P2_AI_Confirmed);

        let fourth = queue.pop().unwrap();
        assert_eq!(fourth.priority, Priority::P3_AI_Autonomous);

        assert!(queue.is_empty());
    }

    // ---- FIFO within same priority ----

    #[test]
    fn fifo_within_same_priority() {
        let queue = PriorityInputQueue::new();

        queue.push(make_event(EventSource::Human, Priority::P1_Human, 100));
        queue.push(make_event(EventSource::Human, Priority::P1_Human, 200));
        queue.push(make_event(EventSource::Human, Priority::P1_Human, 300));

        // Sequence numbers should increase monotonically.
        let seqs: Vec<u64> = std::iter::from_fn(|| queue.pop())
            .map(|ev| ev.sequence)
            .collect();

        assert_eq!(seqs, vec![100, 200, 300]);
    }

    // ---- Emergency stop ----

    #[test]
    fn emergency_stop_sets_flag() {
        let queue = PriorityInputQueue::new();
        assert!(!queue.is_stopped());

        queue.emergency_stop();

        assert!(queue.is_stopped());
    }

    #[test]
    fn emergency_stop_drains_queue() {
        let queue = PriorityInputQueue::new();

        queue.push(make_event(EventSource::Human, Priority::P1_Human, 1));
        queue.push(make_event(EventSource::AI, Priority::P2_AI_Confirmed, 2));
        queue.push(make_event(EventSource::AI, Priority::P3_AI_Autonomous, 3));

        assert_eq!(queue.len(), 3);

        queue.emergency_stop();

        assert!(queue.is_empty());
        assert!(queue.pop().is_none());
    }

    #[test]
    fn push_after_stop_is_ignored() {
        let queue = PriorityInputQueue::new();
        queue.emergency_stop();

        queue.push(make_event(EventSource::Human, Priority::P1_Human, 1));
        assert!(queue.is_empty());
        assert!(queue.pop().is_none());
    }

    #[test]
    fn emergency_stop_can_be_reset_in_tests() {
        let queue = PriorityInputQueue::new();
        queue.emergency_stop();
        assert!(queue.is_stopped());

        queue.reset();
        assert!(!queue.is_stopped());

        queue.push(make_event(EventSource::Human, Priority::P1_Human, 1));
        assert_eq!(queue.len(), 1);
    }

    // ---- Mixed priority + FIFO ----

    #[test]
    fn mixed_priority_and_fifo() {
        let queue = PriorityInputQueue::new();

        // P2 events arrive first.
        queue.push(make_event(EventSource::AI, Priority::P2_AI_Confirmed, 1));
        queue.push(make_event(EventSource::AI, Priority::P2_AI_Confirmed, 2));

        // Then P1.
        queue.push(make_event(EventSource::Human, Priority::P1_Human, 3));
        queue.push(make_event(EventSource::Human, Priority::P1_Human, 4));

        // Then P0 (emergency).
        queue.push(make_event(EventSource::System, Priority::P0_Emergency, 5));

        // Pop order: P0, then P1(3), P1(4), then P2(1), P2(2).
        let order: Vec<(Priority, u64)> = std::iter::from_fn(|| queue.pop())
            .map(|ev| (ev.priority, ev.sequence))
            .collect();

        assert_eq!(
            order,
            vec![
                (Priority::P0_Emergency, 5),
                (Priority::P1_Human, 3),
                (Priority::P1_Human, 4),
                (Priority::P2_AI_Confirmed, 1),
                (Priority::P2_AI_Confirmed, 2),
            ]
        );
    }

    // ---- Default ----

    #[test]
    fn default_queue_is_empty() {
        let queue: PriorityInputQueue = Default::default();
        assert!(queue.is_empty());
        assert!(!queue.is_stopped());
    }

    // ---- Priority → u8 mapping ----

    #[test]
    fn priority_to_u8_values() {
        assert_eq!(priority_to_u8(&Priority::P0_Emergency), 0);
        assert_eq!(priority_to_u8(&Priority::P1_Human), 1);
        assert_eq!(priority_to_u8(&Priority::P2_AI_Confirmed), 2);
        assert_eq!(priority_to_u8(&Priority::P3_AI_Autonomous), 3);
    }
}
