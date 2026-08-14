//! Order-independent cross-player attack arbitration at settlement boundaries.

use crate::nuisance::{OffsetFacts, enqueue};

/// Result of resolving both players' already-formed attacks at one safety point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafetyPointReport {
    pub offsets: [OffsetFacts; 2],
    pub queues_after: [u32; 2],
}

/// Resolves attacks against the queue snapshot, then simultaneously enqueues residuals.
///
/// Both offsets read the queue as it stood on entry, so the participant slot
/// iteration order cannot change the result.
#[must_use]
pub fn arbitrate_attacks(attacks: [u32; 2], queues: [u32; 2]) -> SafetyPointReport {
    arbitrate_attacks_with_limit(attacks, queues, u32::MAX)
}

/// Arbitrates attacks and clamps each queue to the profile's limit.
#[must_use]
pub fn arbitrate_attacks_with_limit(
    attacks: [u32; 2],
    queues: [u32; 2],
    queue_limit: u32,
) -> SafetyPointReport {
    let offsets = std::array::from_fn(|slot| {
        let offset = attacks[slot].min(queues[slot]);
        OffsetFacts {
            offset,
            sent: attacks[slot] - offset,
        }
    });
    let mut queues_after = [queues[0] - offsets[0].offset, queues[1] - offsets[1].offset];
    enqueue(&mut queues_after[0], offsets[1].sent, queue_limit);
    enqueue(&mut queues_after[1], offsets[0].sent, queue_limit);
    SafetyPointReport {
        offsets,
        queues_after,
    }
}
