//! Two-slot match aggregation root, its safety point, and BO3 progression.

use crate::{
    board::Board,
    input::TickInputs,
    match_spec::{LockedMatchSpec, PARTICIPANT_SLOTS},
    nuisance::OffsetFacts,
    player::{PlayerBattleState, PlayerSettlement},
};

/// Result of one round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundOutcome {
    /// One participant won.
    Decided(usize),
    /// Both lost in the same check, so the round is replayed.
    Draw,
}

/// Result of the whole match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchOutcome {
    /// Participant that reached two wins.
    pub winner: usize,
}

/// Match lifecycle visible to callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchPhase {
    /// Countdown before the round's first controllable tick.
    RoundIntro {
        /// Ticks left before play opens.
        remaining_ticks: u16,
    },
    /// Both players are controlling their own boards.
    Playing,
    /// The round result is being shown.
    RoundOutro {
        /// The result being shown.
        outcome: RoundOutcome,
        /// Ticks left before the next round.
        remaining_ticks: u16,
    },
    /// The match is over.
    Completed(MatchOutcome),
}

impl MatchPhase {
    /// Whether gameplay actions are consumed this phase.
    #[must_use]
    pub const fn is_playing(self) -> bool {
        matches!(self, Self::Playing)
    }
}

/// A domain fact produced by one tick.
///
/// Events carry a fixed category order so a report never depends on the order
/// the aggregation root happened to visit its slots in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchEvent {
    /// A group locked and started its settlement.
    GroupLocked(usize),
    /// A turn's chain settled.
    ChainSettled {
        /// Participant slot.
        slot: usize,
        /// Number of links the chain reached.
        links: u8,
        /// Whether the visible board ended empty.
        all_clear: bool,
    },
    /// An attack was arbitrated at a safety point.
    AttackArbitrated {
        /// Attacking slot.
        slot: usize,
        /// What the attack cancelled from the attacker's own queues.
        offset: u32,
        /// What was sent to the opponent.
        sent: u32,
    },
    /// A participant switched to the Fever channel.
    FeverEntered(usize),
    /// A participant returned to the normal channel.
    FeverExited(usize),
    /// A participant's Fever board switched to its next puzzle.
    FeverPuzzleAdvanced(usize),
    /// A nuisance batch landed.
    NuisanceDropped {
        /// Participant slot.
        slot: usize,
        /// Balls released.
        count: u32,
    },
    /// A participant lost the round.
    PlayerDefeated(usize),
    /// A round ended.
    RoundEnded(RoundOutcome),
    /// The match ended.
    MatchEnded(MatchOutcome),
}

impl MatchEvent {
    /// Fixed category rank; ties break on participant slot.
    const fn category(&self) -> u8 {
        match self {
            Self::GroupLocked(_) => 0,
            Self::ChainSettled { .. } => 1,
            Self::AttackArbitrated { .. } => 2,
            Self::FeverEntered(_) => 3,
            Self::FeverExited(_) => 4,
            Self::FeverPuzzleAdvanced(_) => 5,
            Self::NuisanceDropped { .. } => 6,
            Self::PlayerDefeated(_) => 7,
            Self::RoundEnded(_) => 8,
            Self::MatchEnded(_) => 9,
        }
    }

    const fn slot(&self) -> usize {
        match self.participant() {
            Some(slot) => slot,
            None => 0,
        }
    }

    /// The participant a fact belongs to, or `None` for a match-level fact.
    #[must_use]
    pub const fn participant(&self) -> Option<usize> {
        match self {
            Self::GroupLocked(slot)
            | Self::PlayerDefeated(slot)
            | Self::FeverEntered(slot)
            | Self::FeverExited(slot)
            | Self::FeverPuzzleAdvanced(slot) => Some(*slot),
            Self::ChainSettled { slot, .. }
            | Self::AttackArbitrated { slot, .. }
            | Self::NuisanceDropped { slot, .. } => Some(*slot),
            Self::RoundEnded(_) | Self::MatchEnded(_) => None,
        }
    }
}

/// A read-only result from consuming one fixed input tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchStepReport {
    /// Ticks consumed since the match began.
    pub match_tick: u64,
    /// Lifecycle phase after this tick.
    pub phase: MatchPhase,
    /// Facts produced by this tick, in category then slot order.
    pub events: Vec<MatchEvent>,
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

/// One round's mutable state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundState {
    round_tick: u64,
    players: [PlayerBattleState; PARTICIPANT_SLOTS],
    outcome: Option<RoundOutcome>,
}

impl RoundState {
    fn new(spec: &LockedMatchSpec, round_index: u32, draw_attempt: u32) -> Self {
        Self {
            round_tick: 0,
            players: std::array::from_fn(|slot| {
                PlayerBattleState::new(spec, slot, round_index, draw_attempt)
            }),
            outcome: None,
        }
    }

    /// Ticks consumed since this round opened.
    #[must_use]
    pub const fn round_tick(&self) -> u64 {
        self.round_tick
    }

    /// One participant's state.
    #[must_use]
    pub fn player(&self, slot: usize) -> Option<&PlayerBattleState> {
        self.players.get(slot)
    }

    /// One participant's state, mutably. For verification tooling and tests.
    pub fn player_mut(&mut self, slot: usize) -> Option<&mut PlayerBattleState> {
        self.players.get_mut(slot)
    }
}

/// All mutable rules state for a two-player BO3.
///
/// The aggregation root is the only owner of cross-player writes, so a
/// participant can never reach the opposing slot directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchState {
    spec: LockedMatchSpec,
    match_tick: u64,
    phase: MatchPhase,
    wins: [u8; PARTICIPANT_SLOTS],
    round_index: u32,
    draw_attempt: u32,
    round_history: Vec<RoundOutcome>,
    round: RoundState,
}

impl MatchState {
    /// Starts the first round from immutable frozen match data.
    #[must_use]
    pub fn new(spec: LockedMatchSpec) -> Self {
        let round = RoundState::new(&spec, 0, 0);
        Self {
            phase: MatchPhase::RoundIntro {
                remaining_ticks: spec.round_intro_ticks,
            },
            spec,
            match_tick: 0,
            wins: [0; PARTICIPANT_SLOTS],
            round_index: 0,
            draw_attempt: 0,
            round_history: Vec::new(),
            round,
        }
    }

    /// Frozen selection that governs this whole match.
    #[must_use]
    pub const fn spec(&self) -> &LockedMatchSpec {
        &self.spec
    }

    /// Current match tick, including intro and outro ticks.
    #[must_use]
    pub const fn match_tick(&self) -> u64 {
        self.match_tick
    }

    /// Current lifecycle phase.
    #[must_use]
    pub const fn phase(&self) -> MatchPhase {
        self.phase
    }

    /// Rounds won per participant.
    #[must_use]
    pub const fn wins(&self) -> [u8; PARTICIPANT_SLOTS] {
        self.wins
    }

    /// Zero-based round number.
    #[must_use]
    pub const fn round_index(&self) -> u32 {
        self.round_index
    }

    /// How many times this round number has been replayed after a draw.
    #[must_use]
    pub const fn draw_attempt(&self) -> u32 {
        self.draw_attempt
    }

    /// Results of the rounds played so far, including draws.
    #[must_use]
    pub fn round_history(&self) -> &[RoundOutcome] {
        &self.round_history
    }

    /// The match result, once one participant has two wins.
    #[must_use]
    pub const fn outcome(&self) -> Option<MatchOutcome> {
        match self.phase {
            MatchPhase::Completed(outcome) => Some(outcome),
            _ => None,
        }
    }

    /// The current round.
    #[must_use]
    pub const fn round(&self) -> &RoundState {
        &self.round
    }

    /// The current round, mutably. For verification tooling and tests.
    pub const fn round_mut(&mut self) -> &mut RoundState {
        &mut self.round
    }

    /// Board of one participant's active channel.
    #[must_use]
    pub fn board(&self, slot: usize) -> Option<&Board> {
        Some(self.round.player(slot)?.board())
    }

    /// Currently controllable group for one participant.
    #[must_use]
    pub fn active_group(&self, slot: usize) -> Option<&crate::falling::FallingGroup> {
        self.round.player(slot)?.active_group()
    }

    /// Supply state for one participant.
    #[must_use]
    pub fn stream(&self, slot: usize) -> Option<&crate::drop_stream::DropStream> {
        Some(self.round.player(slot)?.stream())
    }

    /// Player-level Fever state.
    #[must_use]
    pub fn fever(&self, slot: usize) -> Option<crate::fever::FeverState> {
        Some(self.round.player(slot)?.fever())
    }

    /// Whether a participant has already lost the current round.
    #[must_use]
    pub fn is_defeated(&self, slot: usize) -> bool {
        self.round
            .player(slot)
            .is_some_and(PlayerBattleState::is_defeated)
    }

    /// Consumes exactly one two-slot input tick.
    ///
    /// Both players advance from the same starting snapshot; nothing that
    /// crosses the slot boundary happens until the safety point.
    pub fn step(&mut self, inputs: &TickInputs) -> Result<MatchStepReport, MatchStepError> {
        if inputs.len() != PARTICIPANT_SLOTS {
            return Err(MatchStepError::ParticipantCount {
                expected: PARTICIPANT_SLOTS,
                actual: inputs.len(),
            });
        }
        self.match_tick += 1;
        let mut events = Vec::new();

        match self.phase {
            MatchPhase::Completed(_) => {
                return Ok(self.report(events));
            }
            MatchPhase::RoundIntro { remaining_ticks } => {
                // Only the deterministic countdown advances; gameplay actions
                // are consumed and discarded rather than held over.
                let remaining = remaining_ticks.saturating_sub(1);
                self.phase = if remaining == 0 {
                    MatchPhase::Playing
                } else {
                    MatchPhase::RoundIntro {
                        remaining_ticks: remaining,
                    }
                };
                return Ok(self.report(events));
            }
            MatchPhase::RoundOutro {
                outcome,
                remaining_ticks,
            } => {
                let remaining = remaining_ticks.saturating_sub(1);
                if remaining == 0 {
                    self.open_next_round(outcome, &mut events);
                } else {
                    self.phase = MatchPhase::RoundOutro {
                        outcome,
                        remaining_ticks: remaining,
                    };
                }
                return Ok(self.report(events));
            }
            MatchPhase::Playing => {}
        }

        self.round.round_tick += 1;
        let round_tick = self.round.round_tick;
        for slot in 0..PARTICIPANT_SLOTS {
            let actions = inputs.player(slot).unwrap_or_default();
            if self.round.players[slot].step(&self.spec, actions, round_tick) {
                events.push(MatchEvent::GroupLocked(slot));
            }
        }

        let settlements: [Option<PlayerSettlement>; PARTICIPANT_SLOTS] =
            std::array::from_fn(|slot| self.round.players[slot].take_settlement());
        if settlements.iter().any(Option::is_some) {
            self.run_safety_point(settlements, &mut events);
        }
        self.supply_and_conclude(&mut events);

        Ok(self.report(events))
    }

    /// The first four ordered steps of a safety point; the last two are
    /// [`MatchState::supply_and_conclude`].
    ///
    /// The order lives in this one function rather than in system registration
    /// order, and every cross-player value is read from the snapshot taken on
    /// entry, so the slot iteration order cannot change the result.
    fn run_safety_point(
        &mut self,
        settlements: [Option<PlayerSettlement>; PARTICIPANT_SLOTS],
        events: &mut Vec<MatchEvent>,
    ) {
        // 1. Collect both reports.
        for (slot, settlement) in settlements.iter().enumerate() {
            if let Some(settlement) = settlement {
                events.push(MatchEvent::ChainSettled {
                    slot,
                    links: settlement.report.links.len() as u8,
                    all_clear: settlement.report.field.all_clear,
                });
            }
        }

        // 2. Arbitrate attacks against the queues as they stood on entry.
        let attacks: [u32; PARTICIPANT_SLOTS] = std::array::from_fn(|slot| {
            settlements[slot]
                .as_ref()
                .map_or(0, |settlement| settlement.attack)
        });
        let offsets: [OffsetFacts; PARTICIPANT_SLOTS] =
            std::array::from_fn(|slot| self.round.players[slot].offset(attacks[slot]));
        for (slot, facts) in offsets.iter().enumerate() {
            if attacks[slot] > 0 {
                events.push(MatchEvent::AttackArbitrated {
                    slot,
                    offset: facts.offset,
                    sent: facts.sent,
                });
            }
        }

        // The attacker whose chain was offset is the one rewarded time. Inside
        // Fever the reward is held until the last link's preview tick.
        for slot in 0..PARTICIPANT_SLOTS {
            if offsets[slot].offset > 0 && attacks[slot] > 0 {
                let reward = self.spec.fever.offset_reward_ticks;
                let player = &mut self.round.players[slot];
                if player.fever().active() {
                    player.fever_mut().defer_reward(reward);
                } else {
                    player.fever_mut().reward_time(reward);
                }
            }
        }

        // 3. Apply all-clear and Fever transition intents. One priority table,
        //    applied once, so no system ordering can change the result.
        let mut exited = [false; PARTICIPANT_SLOTS];
        for (slot, settlement) in settlements.iter().enumerate() {
            let Some(settlement) = settlement else {
                continue;
            };
            let all_clear = settlement.report.field.all_clear;
            let achieved = settlement.report.links.len() as u8;
            let player = &mut self.round.players[slot];

            player.fever_mut().begin_safety_point();
            // A qualifying offset is one an effective chain produced.
            player
                .fever_mut()
                .record_offset(offsets[slot].offset > 0 && settlement.triggered_chain());
            if all_clear {
                let reward = self.spec.fever.all_clear_reward_ticks;
                player.fever_mut().reward_time(reward);
            }

            let in_fever = player.fever().active();
            if in_fever && player.fever().exit_pending() {
                player.exit_fever(&self.spec);
                exited[slot] = true;
                events.push(MatchEvent::FeverExited(slot));
            } else if in_fever {
                if player.advance_fever_puzzle(&self.spec, achieved, all_clear) {
                    events.push(MatchEvent::FeverPuzzleAdvanced(slot));
                }
            } else if player.fever().is_full() {
                let bonus = if all_clear {
                    self.spec.fever.level_ladder.on_all_clear
                } else {
                    0
                };
                if player.enter_fever(&self.spec, bonus) {
                    events.push(MatchEvent::FeverEntered(slot));
                }
            } else if all_clear {
                // A normal-board all clear immediately loads the preset puzzle.
                player.load_all_clear_puzzle(&self.spec);
            }
        }

        // 4. Release nuisance, still from the entry-time queue.
        for (slot, settlement) in settlements.iter().enumerate() {
            let Some(settlement) = settlement else {
                continue;
            };
            // Flipping back with nothing offset triggers one release right
            // away, which still obeys the single-batch limit.
            let force_release = exited[slot] && offsets[slot].offset == 0;
            let triggered = settlement.triggered_chain() && !force_release;
            if let Some(landing) = self.round.players[slot].release(&self.spec, triggered) {
                events.push(MatchEvent::NuisanceDropped {
                    slot,
                    count: landing.dropped,
                });
            }
        }

        // Only now does what this safety point produced reach the opponent, so
        // it is neither offset nor dropped until the next one.
        let limit = self.spec.nuisance.queue_limit;
        for (slot, facts) in offsets.iter().enumerate() {
            let opponent = (slot + 1) % PARTICIPANT_SLOTS;
            if facts.sent > 0 {
                self.round.players[opponent].receive(facts.sent, limit);
            }
        }
    }

    /// Steps 5 and 6 of the safety point, run once per playing tick.
    ///
    /// Supplying the next group is the only way to lose, so this is also the
    /// defeat check. A player whose released batch is still falling is not
    /// waiting for a group yet and reaches this step on the tick that batch
    /// lands instead, which is why the two steps are not inside the safety
    /// point that released it.
    fn supply_and_conclude(&mut self, events: &mut Vec<MatchEvent>) {
        // 5. Check defeat.
        let mut defeated = [false; PARTICIPANT_SLOTS];
        for (slot, lost) in defeated.iter_mut().enumerate() {
            if !self.round.players[slot].needs_group() {
                continue;
            }
            if !self.round.players[slot].supply_next(&self.spec) {
                *lost = true;
                events.push(MatchEvent::PlayerDefeated(slot));
            }
        }

        // 6. Form the round outcome.
        let outcome = match defeated {
            [true, true] => Some(RoundOutcome::Draw),
            [true, false] => Some(RoundOutcome::Decided(1)),
            [false, true] => Some(RoundOutcome::Decided(0)),
            [false, false] => None,
        };
        if let Some(outcome) = outcome {
            self.round.outcome = Some(outcome);
            events.push(MatchEvent::RoundEnded(outcome));
            self.phase = MatchPhase::RoundOutro {
                outcome,
                remaining_ticks: self.spec.round_outro_ticks,
            };
        }
    }

    /// Applies a finished round's result and opens whatever comes next.
    fn open_next_round(&mut self, outcome: RoundOutcome, events: &mut Vec<MatchEvent>) {
        self.round_history.push(outcome);
        match outcome {
            RoundOutcome::Draw => {
                // A draw leaves the score and the round number alone and
                // replays the same round with a different sequence.
                self.draw_attempt += 1;
            }
            RoundOutcome::Decided(winner) => {
                self.wins[winner] += 1;
                self.round_index += 1;
                self.draw_attempt = 0;
            }
        }

        if let Some(winner) = self.wins.iter().position(|wins| *wins >= 2) {
            let outcome = MatchOutcome { winner };
            self.phase = MatchPhase::Completed(outcome);
            events.push(MatchEvent::MatchEnded(outcome));
            return;
        }

        self.round = RoundState::new(&self.spec, self.round_index, self.draw_attempt);
        self.phase = MatchPhase::RoundIntro {
            remaining_ticks: self.spec.round_intro_ticks,
        };
    }

    fn report(&self, mut events: Vec<MatchEvent>) -> MatchStepReport {
        events.sort_by_key(|event| (event.category(), event.slot()));
        MatchStepReport {
            match_tick: self.match_tick,
            phase: self.phase,
            events,
        }
    }
}

impl crate::digest::Digestible for RoundOutcome {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        match self {
            Self::Decided(slot) => {
                writer.u8(0);
                writer.len(*slot);
            }
            Self::Draw => writer.u8(1),
        }
    }
}

impl crate::digest::Digestible for MatchPhase {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        match self {
            Self::RoundIntro { remaining_ticks } => {
                writer.u8(0);
                writer.u16(*remaining_ticks);
            }
            Self::Playing => writer.u8(1),
            Self::RoundOutro {
                outcome,
                remaining_ticks,
            } => {
                writer.u8(2);
                outcome.digest_into(writer);
                writer.u16(*remaining_ticks);
            }
            Self::Completed(outcome) => {
                writer.u8(3);
                writer.len(outcome.winner);
            }
        }
    }
}

impl crate::digest::Digestible for RoundState {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.u64(self.round_tick);
        for player in &self.players {
            player.digest_into(writer);
        }
        match &self.outcome {
            Some(outcome) => {
                writer.u8(1);
                outcome.digest_into(writer);
            }
            None => writer.u8(0),
        }
    }
}

impl crate::digest::Digestible for MatchState {
    /// Encodes every persistent field, and nothing presentational.
    ///
    /// The frozen rule text is not re-encoded: the root digest already covers
    /// it and enters the checksum as a prefix.
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.u64(self.match_tick);
        self.phase.digest_into(writer);
        writer.u8(self.wins[0]);
        writer.u8(self.wins[1]);
        writer.u32(self.round_index);
        writer.u32(self.draw_attempt);
        writer.seq(&self.round_history);
        self.round.digest_into(writer);
    }
}
