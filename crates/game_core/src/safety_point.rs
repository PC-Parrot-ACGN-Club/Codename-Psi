//! Order-independent cross-player attack arbitration at settlement boundaries.

use crate::nuisance::OffsetFacts;

/// Result of resolving both players' already-formed attacks at one safety point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SafetyPointReport {
    pub offsets: [OffsetFacts; 2],
    pub queues_after: [u32; 2],
}

/// Resolves attacks against the queue snapshot, then simultaneously enqueues residuals.
#[must_use]
pub fn arbitrate_attacks(attacks: [u32; 2], queues: [u32; 2]) -> SafetyPointReport {
    let offsets = std::array::from_fn(|slot| {
        let offset = attacks[slot].min(queues[slot]);
        OffsetFacts {
            offset,
            sent: attacks[slot] - offset,
        }
    });
    let remaining = [queues[0] - offsets[0].offset, queues[1] - offsets[1].offset];
    SafetyPointReport {
        offsets,
        queues_after: [
            remaining[0] + offsets[1].sent,
            remaining[1] + offsets[0].sent,
        ],
    }
}
