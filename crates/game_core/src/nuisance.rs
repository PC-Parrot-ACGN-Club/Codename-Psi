//! Exact nuisance queues, offsetting, and deterministic drop-column state.

/// Maximum nuisance count released by one no-chain drop.
pub const MAX_NUISANCE_DROP: u32 = 30;
/// Number of board columns used by the rules kernel.
pub const BOARD_COLUMNS: u8 = 6;

/// Frozen nuisance values selected from a rule profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NuisanceRules {
    pub drop_limit: u32,
    pub columns: u8,
}

impl Default for NuisanceRules {
    fn default() -> Self {
        Self {
            drop_limit: MAX_NUISANCE_DROP,
            columns: BOARD_COLUMNS,
        }
    }
}

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
    drop_nuisance_with_rules(pending, state, NuisanceRules::default())
}

/// Removes a bounded batch using profile-provided board geometry.
///
/// `columns == 0` is rejected by profile validation. This function treats it
/// as a no-op defensively so malformed caller data cannot panic a match.
pub fn drop_nuisance_with_rules(
    pending: &mut u32,
    state: &mut NuisanceDropState,
    rules: NuisanceRules,
) -> NuisanceDrop {
    if rules.columns == 0 {
        return NuisanceDrop {
            dropped: 0,
            columns: Vec::new(),
            remaining: *pending,
        };
    }
    let dropped = (*pending).min(rules.drop_limit);
    *pending -= dropped;
    let full_rows = dropped / u32::from(rules.columns);
    let remainder = (dropped % u32::from(rules.columns)) as u8;
    let mut columns = Vec::with_capacity(dropped as usize);
    for _ in 0..full_rows {
        columns.extend(0..rules.columns);
    }
    for offset in 0..remainder {
        columns.push((state.next_column + offset) % rules.columns);
    }
    if remainder == 1 {
        state.next_column = (state.next_column + 1) % rules.columns;
    } else if remainder >= 2 {
        state.next_column = (state.next_column + remainder - 1) % rules.columns;
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
