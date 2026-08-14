//! Two-slot match aggregation root and its deterministic tick boundary.

use crate::{
    board::Board,
    control::{ControlOutcome, ControlRules, ControlState, SplitState},
    determinism::{MatchRng, StreamName},
    drop_stream::{DropStream, spawn_group},
    falling::FallingGroup,
    fever::FeverState,
    input::TickInputs,
    match_spec::{LockedMatchSpec, PARTICIPANT_SLOTS},
    resolution::{ResolutionPhase, ResolutionState},
};

/// Match lifecycle visible to callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchPhase {
    /// Pre-round countdown; gameplay actions are ignored.
    RoundIntro,
    /// Both players are controlling their own boards.
    Playing,
    /// The match is over.
    Completed,
}

/// A read-only result from consuming one fixed input tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchStepReport {
    /// Ticks consumed since the match began.
    pub match_tick: u64,
    /// Lifecycle phase after this tick.
    pub phase: MatchPhase,
    /// Participants whose spawn failed on this tick, in slot order.
    pub spawn_failures: Vec<usize>,
}

/// Refusal that leaves the aggregation root untouched.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MatchStepError {
    /// The tick did not carry exactly one input per participant slot.
    #[error("a match requires exactly {expected} participant inputs, got {actual}")]
    ParticipantCount {
        /// Slots the match has.
        expected: usize,
        /// Slots the caller supplied.
        actual: usize,
    },
}

/// All mutable rules state for a two-player BO3.
///
/// The aggregation root is the only owner of cross-player writes. Per-player
/// components advance their own board and never reach the opposing slot.
#[derive(Debug, Clone)]
pub struct MatchState {
    spec: LockedMatchSpec,
    match_tick: u64,
    round_index: u32,
    draw_attempt: u32,
    phase: MatchPhase,
    boards: [Board; PARTICIPANT_SLOTS],
    color_rng: [MatchRng; PARTICIPANT_SLOTS],
    streams: [DropStream; PARTICIPANT_SLOTS],
    active: [Option<FallingGroup>; PARTICIPANT_SLOTS],
    control: [ControlState; PARTICIPANT_SLOTS],
    split: [Option<SplitState>; PARTICIPANT_SLOTS],
    resolution: [Option<ResolutionState>; PARTICIPANT_SLOTS],
    fever: [FeverState; PARTICIPANT_SLOTS],
    defeated: [bool; PARTICIPANT_SLOTS],
}

impl MatchState {
    /// Starts the first round from immutable frozen match data.
    #[must_use]
    pub fn new(spec: LockedMatchSpec) -> Self {
        let boards = std::array::from_fn(|_| Board::with_geometry(spec.board_geometry));
        let mut color_rng: [MatchRng; PARTICIPANT_SLOTS] = std::array::from_fn(|slot| {
            MatchRng::derive(spec.root_seed, 0, 0, slot as u8, StreamName::Color)
        });
        let streams = std::array::from_fn(|slot| {
            DropStream::new(
                spec.drop_sets[slot].clone(),
                spec.drop.next_queue_len,
                spec.color_count,
                &mut color_rng[slot],
            )
        });
        let fever = [FeverState::new(
            spec.fever.gauge_capacity,
            spec.fever.initial_time_ticks,
            spec.fever.min_time_ticks,
            spec.fever.max_time_ticks,
        ); PARTICIPANT_SLOTS];
        let mut state = Self {
            spec,
            match_tick: 0,
            round_index: 0,
            draw_attempt: 0,
            phase: MatchPhase::RoundIntro,
            boards,
            color_rng,
            streams,
            active: [None, None],
            control: [ControlState::new(); PARTICIPANT_SLOTS],
            split: [None, None],
            resolution: [None, None],
            fever,
            defeated: [false; PARTICIPANT_SLOTS],
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

    /// Current match tick, including intro ticks.
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

    /// Control timers for one participant.
    #[must_use]
    pub fn control(&self, slot: usize) -> Option<&ControlState> {
        self.control.get(slot)
    }

    /// Supply state for one participant.
    #[must_use]
    pub fn stream(&self, slot: usize) -> Option<&DropStream> {
        self.streams.get(slot)
    }

    /// Post-lock free fall in progress for one participant.
    #[must_use]
    pub fn split(&self, slot: usize) -> Option<&SplitState> {
        self.split.get(slot)?.as_ref()
    }

    /// Player-level Fever state, independent from the active board channel.
    #[must_use]
    pub fn fever(&self, slot: usize) -> Option<FeverState> {
        self.fever.get(slot).copied()
    }

    /// Whether a participant has already lost this round.
    #[must_use]
    pub fn is_defeated(&self, slot: usize) -> bool {
        self.defeated.get(slot).copied().unwrap_or(false)
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
                spawn_failures: Vec::new(),
            });
        }

        let mut spawn_failures = Vec::new();
        for slot in 0..PARTICIPANT_SLOTS {
            if self.defeated[slot] {
                continue;
            }
            // Post-lock free fall runs before anything can scan the board, so
            // a ball still in flight never enters a link.
            if let Some(split) = &mut self.split[slot] {
                split.tick(&mut self.boards[slot]);
                if split.is_complete() {
                    self.split[slot] = None;
                    self.start_resolution(slot);
                }
                continue;
            }
            if let Some(resolution) = &mut self.resolution[slot] {
                resolution.tick();
                if matches!(resolution.phase(), ResolutionPhase::Settlement(_)) {
                    self.boards[slot] = resolution.board().clone();
                    self.resolution[slot] = None;
                    if !self.spawn_next(slot) {
                        spawn_failures.push(slot);
                    }
                }
                continue;
            }

            let outcome = {
                let rules = ControlRules {
                    timing: &self.spec.drop,
                    fall_ticks_by_distance: &self.spec.resolution.gravity_ticks_by_distance,
                    color_count: self.spec.color_count,
                };
                match &mut self.active[slot] {
                    Some(group) => self.control[slot].step(
                        group,
                        &mut self.boards[slot],
                        inputs.player(slot).unwrap_or_default(),
                        rules,
                    ),
                    None => ControlOutcome::Continue,
                }
            };
            if let ControlOutcome::Locked { split, .. } = outcome {
                self.active[slot] = None;
                match split {
                    Some(split) => self.split[slot] = Some(split),
                    None => self.start_resolution(slot),
                }
            }
        }
        Ok(MatchStepReport {
            match_tick: self.match_tick,
            phase: self.phase,
            spawn_failures,
        })
    }

    fn start_resolution(&mut self, slot: usize) {
        self.resolution[slot] = Some(ResolutionState::new(
            self.boards[slot].clone(),
            self.spec.resolution.clone(),
        ));
    }

    /// Supplies the next group, or reports the spawn failure that ends a round.
    ///
    /// The spawn pose is tested *before* the hand leaves NEXT, so a failure
    /// leaves the cursor and the queue exactly where they were.
    fn spawn_next(&mut self, slot: usize) -> bool {
        match spawn_group(
            &self.boards[slot],
            &mut self.streams[slot],
            self.spec.board_geometry,
            self.spec.color_count,
            &mut self.color_rng[slot],
        ) {
            Ok(group) => {
                self.active[slot] = Some(group);
                self.control[slot] = ControlState::new();
                true
            }
            Err(_) => {
                self.defeated[slot] = true;
                false
            }
        }
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
        self.streams = std::array::from_fn(|slot| {
            DropStream::new(
                self.spec.drop_sets[slot].clone(),
                self.spec.drop.next_queue_len,
                self.spec.color_count,
                &mut self.color_rng[slot],
            )
        });
        self.active = [None, None];
        self.control = [ControlState::new(); PARTICIPANT_SLOTS];
        self.split = [None, None];
        self.resolution = [None, None];
        self.defeated = [false; PARTICIPANT_SLOTS];
        self.phase = MatchPhase::RoundIntro;
        for slot in 0..PARTICIPANT_SLOTS {
            self.spawn_next(slot);
        }
    }
}
