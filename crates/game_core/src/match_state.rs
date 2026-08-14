//! Two-slot match aggregation root and its deterministic tick boundary.

use crate::{
    board::Board,
    determinism::{MatchRng, StreamName},
    input::TickInputs,
    match_spec::{LockedMatchSpec, PARTICIPANT_SLOTS},
};

/// Match lifecycle visible to callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchPhase {
    RoundIntro,
    Playing,
    Completed,
}

/// A read-only result from consuming one fixed input tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchStepReport {
    pub match_tick: u64,
    pub phase: MatchPhase,
}

/// Refusal that leaves the aggregation root untouched.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MatchStepError {
    #[error("a match requires exactly {expected} participant inputs, got {actual}")]
    ParticipantCount { expected: usize, actual: usize },
}

/// All mutable rules state for a two-player BO3.
///
/// The first implementation establishes the only legal cross-player entry
/// point. Falling groups, settlement safety points and Fever extend this root
/// rather than gaining direct access to the opposing player.
#[derive(Debug, Clone)]
pub struct MatchState {
    spec: LockedMatchSpec,
    match_tick: u64,
    round_index: u32,
    draw_attempt: u32,
    phase: MatchPhase,
    boards: [Board; PARTICIPANT_SLOTS],
    color_rng: [MatchRng; PARTICIPANT_SLOTS],
}

impl MatchState {
    /// Starts the first round from immutable frozen match data.
    #[must_use]
    pub fn new(spec: LockedMatchSpec) -> Self {
        let boards = std::array::from_fn(|_| Board::with_geometry(spec.board_geometry));
        let color_rng = std::array::from_fn(|slot| {
            MatchRng::derive(spec.root_seed, 0, 0, slot as u8, StreamName::Color)
        });
        Self {
            spec,
            match_tick: 0,
            round_index: 0,
            draw_attempt: 0,
            phase: MatchPhase::RoundIntro,
            boards,
            color_rng,
        }
    }

    /// Frozen selection that governs this whole match.
    #[must_use]
    pub const fn spec(&self) -> &LockedMatchSpec {
        &self.spec
    }

    /// Current match tick, including intro/outro ticks.
    #[must_use]
    pub const fn match_tick(&self) -> u64 {
        self.match_tick
    }

    /// Current lifecycle phase.
    #[must_use]
    pub const fn phase(&self) -> MatchPhase {
        self.phase
    }

    /// Board visible to one participant's rules sub-state.
    #[must_use]
    pub fn board(&self, slot: usize) -> Option<&Board> {
        self.boards.get(slot)
    }

    /// Consumes exactly one two-slot input tick.
    pub fn step(&mut self, inputs: &TickInputs) -> Result<MatchStepReport, MatchStepError> {
        if inputs.len() != PARTICIPANT_SLOTS {
            return Err(MatchStepError::ParticipantCount {
                expected: PARTICIPANT_SLOTS,
                actual: inputs.len(),
            });
        }
        self.match_tick += 1;
        if self.phase == MatchPhase::RoundIntro {
            self.phase = MatchPhase::Playing;
        }
        Ok(MatchStepReport {
            match_tick: self.match_tick,
            phase: self.phase,
        })
    }

    /// Rebuilds round-local random state after an exactly simultaneous defeat.
    pub fn retry_draw(&mut self) {
        self.draw_attempt += 1;
        self.boards = std::array::from_fn(|_| Board::with_geometry(self.spec.board_geometry));
        self.color_rng = std::array::from_fn(|slot| {
            MatchRng::derive(
                self.spec.root_seed,
                self.round_index,
                self.draw_attempt,
                slot as u8,
                StreamName::Color,
            )
        });
        self.phase = MatchPhase::RoundIntro;
    }
}
