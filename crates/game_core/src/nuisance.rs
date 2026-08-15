//! Exact nuisance queues, offsetting, and deterministic drop-column state.
//!
//! A queue is a single exact integer, never a list of batches: the UI's tiered
//! icons are a projection of that integer, and the integer is the only truth.

use crate::board::{Board, Cell, Coord};
use crate::resolution::GravityMove;

/// Maximum nuisance count released by one no-chain drop.
pub const MAX_NUISANCE_DROP: u32 = 30;
/// Number of board columns used by the rules kernel.
pub const BOARD_COLUMNS: u8 = 6;

/// Largest pending count one channel may hold, before profile data says otherwise.
pub const MAX_PENDING_NUISANCE: u32 = 100_000;

/// Frozen nuisance values selected from a rule profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NuisanceRules {
    /// Largest batch one no-chain drop releases.
    pub drop_limit: u32,
    /// Largest pending count one channel may hold.
    pub queue_limit: u32,
    /// Board columns the release order walks.
    pub columns: u8,
}

impl Default for NuisanceRules {
    fn default() -> Self {
        Self {
            drop_limit: MAX_NUISANCE_DROP,
            queue_limit: MAX_PENDING_NUISANCE,
            columns: BOARD_COLUMNS,
        }
    }
}

/// Adds `amount` to a channel queue, returning what the limit discarded.
///
/// The limit is a profile value checked during validation, so reaching it means
/// the match has gone far past any reachable board state rather than that the
/// arithmetic went wrong.
pub fn enqueue(pending: &mut u32, amount: u32, limit: u32) -> u32 {
    let total = pending.saturating_add(amount);
    *pending = total.min(limit);
    total - *pending
}

/// An attack that has been converted but not yet arbitrated at a safety point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackIntent {
    /// Exact nuisance count this attack carries.
    pub amount: u32,
    /// One-based chain link that produced it.
    pub chain_index: u8,
}

impl AttackIntent {
    /// An attack produced by one committed link.
    #[must_use]
    pub const fn new(amount: u32, chain_index: u8) -> Self {
        Self {
            amount,
            chain_index,
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

/// Where a released nuisance batch entered the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuisanceLanding {
    /// Entry cells in release order, at the top of each column.
    pub coords: Vec<Coord>,
    /// Balls removed from the queue.
    pub dropped: u32,
    /// Pending count after the release.
    pub remaining: u32,
}

/// A released batch on its way down, before the next group is supplied.
///
/// Released nuisance falls through the same gravity flow a chain link's balls
/// do, so it spends the same per-distance duration and the board only changes
/// when the batch comes to rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuisanceFall {
    moves: Vec<GravityMove>,
    target_board: Board,
    elapsed_ticks: u16,
    duration_ticks: u16,
}

impl NuisanceFall {
    /// Plans the fall of a batch sitting at its entry cells.
    ///
    /// Returns `None` when the batch is already at rest, in which case the board
    /// stays as the entry placement left it and nothing has to be timed.
    #[must_use]
    pub fn plan(board: &mut Board, gravity_ticks_by_distance: &[u16]) -> Option<Self> {
        let (moves, target_board, max_distance) = crate::resolution::gravity_plan(board);
        let duration_ticks =
            crate::resolution::gravity_duration(gravity_ticks_by_distance, max_distance);
        if moves.is_empty() || duration_ticks == 0 {
            *board = target_board;
            return None;
        }
        Some(Self {
            moves,
            target_board,
            elapsed_ticks: 0,
            duration_ticks,
        })
    }

    /// Source and destination of every ball still falling.
    #[must_use]
    pub fn moves(&self) -> &[GravityMove] {
        &self.moves
    }

    /// Ticks spent falling so far.
    #[must_use]
    pub const fn elapsed_ticks(&self) -> u16 {
        self.elapsed_ticks
    }

    /// Frozen duration of the fall.
    #[must_use]
    pub const fn duration_ticks(&self) -> u16 {
        self.duration_ticks
    }

    /// Whether the batch has come to rest.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.elapsed_ticks >= self.duration_ticks
    }

    /// Advances one tick.
    pub const fn tick(&mut self) {
        self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
    }

    /// The board the batch comes to rest on, committed atomically.
    #[must_use]
    pub fn into_target_board(self) -> Board {
        self.target_board
    }
}

/// Releases one batch onto the active board, if this turn releases at all.
///
/// A turn that triggered any chain releases nothing: the queue stays put and
/// the player simply gets the next group. That is the continuous-offset rule,
/// not an optimisation.
///
/// The batch enters at the top of its columns rather than at its resting cells,
/// because it still has to fall: [`NuisanceFall`] is what carries it down.
pub fn release_nuisance(
    board: &mut Board,
    pending: &mut u32,
    state: &mut NuisanceDropState,
    rules: NuisanceRules,
    chain_triggered: bool,
) -> Option<NuisanceLanding> {
    if chain_triggered || *pending == 0 {
        return None;
    }
    let batch = drop_nuisance_with_rules(pending, state, rules);
    let mut coords = Vec::with_capacity(batch.columns.len());
    for column in &batch.columns {
        if let Some(coord) = highest_free_cell(board, *column) {
            board.set(coord, Cell::Nuisance);
            coords.push(coord);
        }
    }
    Some(NuisanceLanding {
        coords,
        dropped: batch.dropped,
        remaining: batch.remaining,
    })
}

/// Highest empty cell in a column, or `None` when the column is full.
///
/// A stable board has all its free cells above its stack, so this is where a
/// ball entering the column starts its fall. A column with no room at all
/// swallows the ball, exactly as a column filled to the top always has.
fn highest_free_cell(board: &Board, column: u8) -> Option<Coord> {
    (0..board.geometry().height())
        .filter_map(|y| board.coord(column, y))
        .find(|coord| !board.get(*coord).is_occupied())
}

impl crate::digest::Digestible for NuisanceDropState {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.u8(self.next_column);
    }
}

impl crate::digest::Digestible for NuisanceFall {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.seq(&self.moves);
        // The uncommitted target board is persistent state even though it is
        // never exposed, so it has to enter the checksum.
        self.target_board.digest_into(writer);
        writer.u16(self.elapsed_ticks);
        writer.u16(self.duration_ticks);
    }
}
