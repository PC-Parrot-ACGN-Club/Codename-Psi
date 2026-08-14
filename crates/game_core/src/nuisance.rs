//! Exact nuisance queues, offsetting, and deterministic drop-column state.

/// Maximum nuisance count released by one no-chain drop.
pub const MAX_NUISANCE_DROP: u32 = 30;
/// Number of board columns used by the rules kernel.
pub const BOARD_COLUMNS: u8 = 6;

/// Per-channel deterministic position in the nuisance column sequence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NuisanceDropState {
    next_column: u8,
}

impl NuisanceDropState {
    /// Starts the sequence at `column`.
    #[must_use]
    pub const fn at_column(column: u8) -> Self {
        Self {
            next_column: column % BOARD_COLUMNS,
        }
    }

    /// The column used for the first remaining nuisance ball.
    #[must_use]
    pub const fn next_column(self) -> u8 {
        self.next_column
    }
}

/// The result of releasing a bounded nuisance batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuisanceDrop {
    /// Number removed from the exact pending count.
    pub dropped: u32,
    /// Columns in their deterministic release order.
    pub columns: Vec<u8>,
    /// Pending count after this release.
    pub remaining: u32,
}

/// Removes at most one release batch from `pending`.
///
/// Full rows always start at column zero.  The state only determines the
/// incomplete final row and follows the two documented continuation branches.
pub fn drop_nuisance(pending: &mut u32, state: &mut NuisanceDropState) -> NuisanceDrop {
    let dropped = (*pending).min(MAX_NUISANCE_DROP);
    *pending -= dropped;
    let full_rows = dropped / u32::from(BOARD_COLUMNS);
    let remainder = (dropped % u32::from(BOARD_COLUMNS)) as u8;
    let mut columns = Vec::with_capacity(dropped as usize);
    for _ in 0..full_rows {
        columns.extend(0..BOARD_COLUMNS);
    }
    for offset in 0..remainder {
        columns.push((state.next_column + offset) % BOARD_COLUMNS);
    }
    if remainder == 1 {
        state.next_column = (state.next_column + 1) % BOARD_COLUMNS;
    } else if remainder >= 2 {
        state.next_column = (state.next_column + remainder - 1) % BOARD_COLUMNS;
    }
    NuisanceDrop {
        dropped,
        columns,
        remaining: *pending,
    }
}

/// Facts emitted when an attack cancels queued nuisance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetFacts {
    /// Amount removed from either local queue.
    pub offset: u32,
    /// Attack still destined for the opponent.
    pub sent: u32,
}

/// Cancels `attack` from the active queue first, then the other queue.
pub fn offset_attack(attack: u32, active: &mut u32, other: &mut u32) -> OffsetFacts {
    let from_active = attack.min(*active);
    *active -= from_active;
    let after_active = attack - from_active;
    let from_other = after_active.min(*other);
    *other -= from_other;
    OffsetFacts {
        offset: from_active + from_other,
        sent: after_active - from_other,
    }
}
