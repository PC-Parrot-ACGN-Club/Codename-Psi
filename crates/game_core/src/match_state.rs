//! Two-slot match aggregation root and its deterministic tick boundary.

use crate::{
    board::Board,
    determinism::{MatchRng, StreamName},
    falling::{FallingGroup, GroupBall},
    fever::FeverState,
    input::TickInputs,
    match_spec::{LockedMatchSpec, PARTICIPANT_SLOTS},
    resolution::{ResolutionPhase, ResolutionState},
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
    #[error("active group violated its frozen placement invariant")]
    InvalidGroup,
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
    drop_cursor: [usize; PARTICIPANT_SLOTS],
    active: [Option<FallingGroup>; PARTICIPANT_SLOTS],
    resolution: [Option<ResolutionState>; PARTICIPANT_SLOTS],
    fever: [FeverState; PARTICIPANT_SLOTS],
}

impl MatchState {
    /// Starts the first round from immutable frozen match data.
    #[must_use]
    pub fn new(spec: LockedMatchSpec) -> Self {
        let boards = std::array::from_fn(|_| Board::with_geometry(spec.board_geometry));
        let color_rng = std::array::from_fn(|slot| {
            MatchRng::derive(spec.root_seed, 0, 0, slot as u8, StreamName::Color)
        });
        let fever = [FeverState::new(
            spec.fever_capacity,
            spec.fever_initial_time_ticks,
            spec.fever_min_time_ticks,
            spec.fever_max_time_ticks,
        ); PARTICIPANT_SLOTS];
        let mut state = Self {
            spec,
            match_tick: 0,
            round_index: 0,
            draw_attempt: 0,
            phase: MatchPhase::RoundIntro,
            boards,
            color_rng,
            drop_cursor: [0; PARTICIPANT_SLOTS],
            active: [None, None],
            resolution: [None, None],
            fever,
        };
        for slot in 0..PARTICIPANT_SLOTS {
            state.spawn_next(slot);
        }
        state
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

    /// Currently controllable group for one participant.
    #[must_use]
    pub fn active_group(&self, slot: usize) -> Option<&FallingGroup> {
        self.active.get(slot)?.as_ref()
    }

    /// Player-level Fever state, independent from the active board channel.
    #[must_use]
    pub fn fever(&self, slot: usize) -> Option<FeverState> {
        self.fever.get(slot).copied()
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
            return Ok(MatchStepReport {
                match_tick: self.match_tick,
                phase: self.phase,
            });
        }
        for slot in 0..PARTICIPANT_SLOTS {
            if let Some(resolution) = &mut self.resolution[slot] {
                resolution.tick();
                if matches!(resolution.phase(), ResolutionPhase::Settlement(_)) {
                    self.boards[slot] = resolution.board().clone();
                    self.resolution[slot] = None;
                    self.spawn_next(slot);
                }
                continue;
            }
            if let Some(group) = &mut self.active[slot] {
                if group
                    .apply_actions(
                        &mut self.boards[slot],
                        inputs.player(slot).unwrap_or_default(),
                    )
                    .map_err(|_| MatchStepError::InvalidGroup)?
                    .is_some()
                {
                    self.active[slot] = None;
                    self.resolution[slot] = Some(ResolutionState::new(
                        self.boards[slot].clone(),
                        self.spec.resolution.clone(),
                    ));
                }
            }
        }
        Ok(MatchStepReport {
            match_tick: self.match_tick,
            phase: self.phase,
        })
    }

    fn spawn_next(&mut self, slot: usize) {
        let template = &self.spec.drop_sets[slot].0[self.drop_cursor[slot] % 16];
        self.drop_cursor[slot] += 1;
        let pivot = self.boards[slot].coord(
            self.spec.board_geometry.spawn_column(),
            self.spec.board_geometry.hidden_rows().saturating_sub(1),
        );
        let Some(pivot) = pivot else {
            self.active[slot] = None;
            return;
        };
        let balls = template
            .balls
            .iter()
            .map(|ball| GroupBall {
                dx: ball.dx,
                dy: ball.dy,
                color: (self.color_rng[slot].next_u32() % u32::from(self.spec.color_count)) as u8,
            })
            .collect();
        let group = FallingGroup::new(pivot, balls, self.drop_cursor[slot] as u32).ok();
        self.active[slot] = group.filter(|candidate| candidate.can_place(&self.boards[slot]));
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
