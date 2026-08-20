//! Fever gauge, player-level clock, puzzle session and level ladder.
//!
//! Fever time is a *player* value, not a session value: an all clear outside
//! Fever adds to it too, so it has to survive a session being created and
//! destroyed.

use std::collections::BTreeMap;

use crate::{
    board::{Board, Cell},
    config::{FeverLadderConfig, FeverPuzzle, FeverPuzzleBook},
    determinism::MatchRng,
};

/// Fever state that survives normal/Fever board channel switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeverState {
    gauge: u8,
    capacity: u8,
    time_ticks: u32,
    min_time_ticks: u32,
    max_time_ticks: u32,
    active: bool,
    exit_pending: bool,
    gauge_credited: bool,
    deferred_reward: u32,
}

impl FeverState {
    /// Opening Fever state for a round.
    #[must_use]
    pub const fn new(
        capacity: u8,
        initial_time_ticks: u32,
        min_time_ticks: u32,
        max_time_ticks: u32,
    ) -> Self {
        Self {
            gauge: 0,
            capacity,
            time_ticks: initial_time_ticks,
            min_time_ticks,
            max_time_ticks,
            active: false,
            exit_pending: false,
            gauge_credited: false,
            deferred_reward: 0,
        }
    }

    /// Whether the Fever channel is the active one.
    #[must_use]
    pub const fn active(self) -> bool {
        self.active
    }

    /// Remaining Fever time in ticks.
    #[must_use]
    pub const fn time_ticks(self) -> u32 {
        self.time_ticks
    }

    /// Whole seconds of Fever time, which is what the UI shows.
    #[must_use]
    pub const fn time_seconds(self) -> u32 {
        self.time_ticks / 60
    }

    /// Filled gauge cells.
    #[must_use]
    pub const fn gauge(self) -> u8 {
        self.gauge
    }

    /// Gauge cells needed to enter.
    #[must_use]
    pub const fn capacity(self) -> u8 {
        self.capacity
    }

    /// Whether the clock has run out and the exit is waiting for a boundary.
    ///
    /// The clock does not pause for settlement animation, so it can reach zero
    /// mid-chain. The exit is recorded here and applied at the safety point,
    /// which is what keeps the chain's tick sequence unaffected.
    #[must_use]
    pub const fn exit_pending(self) -> bool {
        self.exit_pending
    }

    /// Opens a safety point, allowing the gauge to take one more cell.
    pub const fn begin_safety_point(&mut self) {
        self.gauge_credited = false;
    }

    /// Records a qualifying offset; one safety point adds at most one cell.
    pub const fn record_offset(&mut self, qualifying: bool) {
        if qualifying && !self.gauge_credited && self.gauge < self.capacity {
            self.gauge += 1;
            self.gauge_credited = true;
        }
    }

    /// Whether the gauge is full and the player may enter at this boundary.
    #[must_use]
    pub const fn is_full(self) -> bool {
        self.gauge >= self.capacity
    }

    /// Enters Fever, resetting the gauge. Callers check the boundary first.
    pub const fn enter(&mut self) {
        self.gauge = 0;
        self.active = true;
        self.exit_pending = false;
    }

    /// Adds a player-level time reward, clamped to the declared upper bound.
    pub const fn reward_time(&mut self, ticks: u32) {
        self.time_ticks = if self.time_ticks.saturating_add(ticks) > self.max_time_ticks {
            self.max_time_ticks
        } else {
            self.time_ticks + ticks
        };
    }

    /// Holds a reward until the chain reaches its last link's preview tick.
    ///
    /// Inside Fever the design pays rewards on that tick rather than at
    /// settlement, so a long chain cannot delay the clock's credit.
    pub const fn defer_reward(&mut self, ticks: u32) {
        self.deferred_reward = self.deferred_reward.saturating_add(ticks);
    }

    /// Pays whatever was deferred, returning the amount paid.
    pub const fn release_deferred_reward(&mut self) -> u32 {
        let ticks = self.deferred_reward;
        self.deferred_reward = 0;
        self.reward_time(ticks);
        ticks
    }

    /// Reward waiting for the last link's preview tick.
    #[must_use]
    pub const fn deferred_reward(self) -> u32 {
        self.deferred_reward
    }

    /// Advances one rules tick; expiry asks the caller to exit at a boundary.
    pub const fn tick(&mut self) -> bool {
        if self.active {
            self.time_ticks = if self.time_ticks > self.min_time_ticks {
                self.time_ticks - 1
            } else {
                self.min_time_ticks
            };
            if self.time_ticks == self.min_time_ticks {
                self.exit_pending = true;
            }
        }
        self.active && self.exit_pending
    }

    /// Applies a pending exit. Callers only reach here at a safety point.
    pub const fn exit(&mut self) {
        self.active = false;
        self.exit_pending = false;
    }
}

/// One Fever session: the target level and the per-level puzzle bags.
///
/// The bags are rules state, so they enter the snapshot: a bag half consumed
/// is part of what makes the next draw reproducible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeverSession {
    target_level: u8,
    current_puzzle_id: String,
    bags: PuzzleBags,
}

impl FeverSession {
    /// Opens a session at a starting target level, drawing the first puzzle.
    #[must_use]
    pub fn open(
        book: &FeverPuzzleBook,
        target_level: u8,
        bags: PuzzleBags,
        rng: &mut MatchRng,
    ) -> Option<Self> {
        let mut bags = bags;
        let puzzle_id = bags.draw(book, target_level, rng)?;
        Some(Self {
            target_level,
            current_puzzle_id: puzzle_id,
            bags,
        })
    }

    /// Target chain the current puzzle asks for.
    #[must_use]
    pub const fn target_level(&self) -> u8 {
        self.target_level
    }

    /// Puzzle currently loaded onto the Fever board.
    #[must_use]
    pub fn current_puzzle_id(&self) -> &str {
        &self.current_puzzle_id
    }

    /// The per-level bags, which are part of rules state.
    #[must_use]
    pub const fn bags(&self) -> &PuzzleBags {
        &self.bags
    }

    /// Moves to the next target level and draws its puzzle.
    pub fn advance(
        &mut self,
        book: &FeverPuzzleBook,
        next_level: u8,
        rng: &mut MatchRng,
    ) -> Option<&str> {
        let puzzle_id = self.bags.draw(book, next_level, rng)?;
        self.target_level = next_level;
        self.current_puzzle_id = puzzle_id;
        Some(&self.current_puzzle_id)
    }
}

/// One no-repeat bag per target level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PuzzleBags {
    remaining: BTreeMap<u8, Vec<String>>,
}

impl PuzzleBags {
    /// Bags that have not been drawn from yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ids still in one level's bag, in canonical order.
    #[must_use]
    pub fn remaining(&self, level: u8) -> &[String] {
        self.remaining.get(&level).map_or(&[], Vec::as_slice)
    }

    /// Draws one puzzle id, refilling the level's bag when it is empty.
    ///
    /// Refilling uses the book's canonical order, so the bag contents never
    /// depend on the order earlier draws happened to take.
    pub fn draw(
        &mut self,
        book: &FeverPuzzleBook,
        level: u8,
        rng: &mut MatchRng,
    ) -> Option<String> {
        let bag = self.remaining.entry(level).or_default();
        if bag.is_empty() {
            bag.extend(
                book.puzzles
                    .iter()
                    .filter(|puzzle| puzzle.level == level)
                    .map(|puzzle| puzzle.id.clone()),
            );
        }
        if bag.is_empty() {
            return None;
        }
        let index = (rng.next_u32() as usize) % bag.len();
        Some(bag.swap_remove(index))
    }
}

/// Applies the level ladder to one puzzle attempt.
///
/// Every branch reads only the achieved chain against the target, never the
/// step's score or attack, and the result is clamped into the declared domain.
#[must_use]
pub fn next_target_level(
    ladder: FeverLadderConfig,
    target_level: u8,
    achieved: u8,
    all_clear: bool,
    min_level: u8,
    max_level: u8,
) -> u8 {
    let target = i32::from(target_level);
    let actual = i32::from(achieved);
    let raw = if actual >= target {
        if all_clear {
            actual + i32::from(ladder.on_all_clear)
        } else {
            actual + i32::from(ladder.on_target)
        }
    } else {
        match target - actual {
            1 => target,
            2 => actual + i32::from(ladder.miss_by_two),
            _ => actual + i32::from(ladder.miss_by_more),
        }
    };
    raw.clamp(i32::from(min_level), i32::from(max_level)) as u8
}

/// Loads a puzzle onto a board, replacing whatever was there.
pub fn load_puzzle(board: &mut Board, puzzle: &FeverPuzzle) {
    let geometry = board.geometry();
    *board = Board::with_geometry(geometry);
    for cell in &puzzle.cells {
        if let Some(coord) = board.coord(cell.x, cell.y) {
            board.set(coord, Cell::Color(cell.color));
        }
    }
}

/// Finds a puzzle by id.
#[must_use]
pub fn puzzle_by_id<'a>(book: &'a FeverPuzzleBook, id: &str) -> Option<&'a FeverPuzzle> {
    book.puzzles.iter().find(|puzzle| puzzle.id == id)
}

impl crate::digest::Digestible for FeverState {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.u8(self.gauge);
        writer.u8(self.capacity);
        writer.u32(self.time_ticks);
        writer.u32(self.min_time_ticks);
        writer.u32(self.max_time_ticks);
        writer.bool(self.active);
        writer.bool(self.exit_pending);
        writer.bool(self.gauge_credited);
        writer.u32(self.deferred_reward);
    }
}

impl crate::digest::Digestible for PuzzleBags {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.len(self.remaining.len());
        for (level, ids) in &self.remaining {
            writer.u8(*level);
            writer.seq(ids);
        }
    }
}

impl crate::digest::Digestible for FeverSession {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.u8(self.target_level);
        writer.str(&self.current_puzzle_id);
        self.bags.digest_into(writer);
    }
}
