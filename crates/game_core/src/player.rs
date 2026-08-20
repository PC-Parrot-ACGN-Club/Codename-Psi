//! One participant's whole rules state.
//!
//! A player advances its own board, resolution and timers, and forms the facts
//! a safety point later arbitrates. It can never reach the opposing slot: every
//! cross-player effect goes through the aggregation root.

use crate::{
    board::Board,
    control::{ControlOutcome, ControlRules, ControlState, SplitState},
    determinism::{MatchRng, StreamName},
    drop_stream::{DropStream, spawn_group},
    falling::FallingGroup,
    fever::{FeverSession, FeverState, PuzzleBags, load_puzzle, next_target_level, puzzle_by_id},
    input::PlayerActions,
    match_spec::LockedMatchSpec,
    nuisance::{NuisanceDropState, NuisanceFall, NuisanceLanding, release_nuisance},
    resolution::{ChainReport, ResolutionPhase, ResolutionState},
    rules::BoardMode,
    scoring::{AttackFraction, MarginState, ScoreState},
};

/// The two board channels a player owns.
pub const CHANNELS: usize = 2;
/// Index of the ordinary board channel.
pub const NORMAL_CHANNEL: usize = 0;
/// Index of the Fever board channel.
pub const FEVER_CHANNEL: usize = 1;

/// Facts a finished turn hands to the safety point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSettlement {
    /// Exact nuisance this turn's chain converted to.
    pub attack: u32,
    /// The turn's complete chain report.
    pub report: ChainReport,
}

impl PlayerSettlement {
    /// Whether the turn triggered at least one link.
    #[must_use]
    pub fn triggered_chain(&self) -> bool {
        !self.report.links.is_empty()
    }
}

/// All rules state belonging to one participant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerBattleState {
    active_channel: usize,
    boards: [Board; CHANNELS],
    pending: [u32; CHANNELS],
    drop_state: [NuisanceDropState; CHANNELS],
    stream: DropStream,
    active: Option<FallingGroup>,
    control: ControlState,
    split: Option<SplitState>,
    nuisance_fall: Option<NuisanceFall>,
    resolution: Option<ResolutionState>,
    score: ScoreState,
    fraction: AttackFraction,
    margin: MarginState,
    fever: FeverState,
    session: Option<FeverSession>,
    bags: PuzzleBags,
    color_rng: MatchRng,
    nuisance_rng: MatchRng,
    fever_rng: MatchRng,
    chain_total_links: u8,
    pending_attack: u32,
    settlement: Option<PlayerSettlement>,
    defeated: bool,
    chain_power_slot: usize,
}

impl PlayerBattleState {
    /// Builds a player's opening state for one round.
    #[must_use]
    pub fn new(spec: &LockedMatchSpec, slot: usize, round_index: u32, draw_attempt: u32) -> Self {
        let stream_for =
            |name| MatchRng::derive(spec.root_seed, round_index, draw_attempt, slot as u8, name);
        let mut color_rng = stream_for(StreamName::Color);
        let stream = DropStream::new(
            spec.drop_sets[slot].clone(),
            spec.drop.next_queue_len,
            spec.color_count,
            &mut color_rng,
        );
        let mut player = Self {
            active_channel: NORMAL_CHANNEL,
            boards: std::array::from_fn(|_| Board::with_geometry(spec.board_geometry)),
            pending: [0; CHANNELS],
            drop_state: [NuisanceDropState::default(); CHANNELS],
            stream,
            active: None,
            control: ControlState::new(),
            split: None,
            nuisance_fall: None,
            resolution: None,
            score: ScoreState::default(),
            fraction: AttackFraction::default(),
            margin: MarginState::default(),
            fever: FeverState::new(
                spec.fever.gauge_capacity,
                spec.fever.initial_time_ticks,
                spec.fever.min_time_ticks,
                spec.fever.max_time_ticks,
            ),
            session: None,
            bags: PuzzleBags::new(),
            color_rng,
            nuisance_rng: stream_for(StreamName::Nuisance),
            fever_rng: stream_for(StreamName::FeverPuzzle),
            chain_total_links: 0,
            pending_attack: 0,
            settlement: None,
            defeated: false,
            chain_power_slot: slot,
        };
        player.supply_next(spec);
        player
    }

    /// The board this player is currently dropping onto.
    #[must_use]
    pub fn board(&self) -> &Board {
        &self.boards[self.active_channel]
    }

    /// One of the player's two channel boards.
    #[must_use]
    pub fn channel_board(&self, channel: usize) -> Option<&Board> {
        self.boards.get(channel)
    }

    /// Replaces the active channel's board. For verification tooling and tests.
    pub fn set_board(&mut self, board: Board) {
        self.boards[self.active_channel] = board;
    }

    /// Which channel is active.
    #[must_use]
    pub const fn active_channel(&self) -> usize {
        self.active_channel
    }

    /// Pending nuisance on one channel.
    #[must_use]
    pub fn pending(&self, channel: usize) -> u32 {
        self.pending.get(channel).copied().unwrap_or(0)
    }

    /// Sets a channel's pending nuisance. For verification tooling and tests.
    pub fn set_pending(&mut self, channel: usize, amount: u32) {
        if let Some(slot) = self.pending.get_mut(channel) {
            *slot = amount;
        }
    }

    /// Column-order position of one channel.
    #[must_use]
    pub fn drop_state(&self, channel: usize) -> NuisanceDropState {
        self.drop_state
            .get(channel)
            .copied()
            .unwrap_or_else(NuisanceDropState::default)
    }

    /// Currently controllable group.
    #[must_use]
    pub const fn active_group(&self) -> Option<&FallingGroup> {
        self.active.as_ref()
    }

    /// Control timers.
    #[must_use]
    pub const fn control(&self) -> &ControlState {
        &self.control
    }

    /// Supply state.
    #[must_use]
    pub const fn stream(&self) -> &DropStream {
        &self.stream
    }

    /// Post-lock free fall in progress.
    #[must_use]
    pub const fn split(&self) -> Option<&SplitState> {
        self.split.as_ref()
    }

    /// Released nuisance still falling.
    #[must_use]
    pub const fn nuisance_fall(&self) -> Option<&NuisanceFall> {
        self.nuisance_fall.as_ref()
    }

    /// Chain resolution in progress.
    #[must_use]
    pub const fn resolution(&self) -> Option<&ResolutionState> {
        self.resolution.as_ref()
    }

    /// Score, display and attack parts separately.
    #[must_use]
    pub const fn score(&self) -> ScoreState {
        self.score
    }

    /// Carried attack remainder.
    #[must_use]
    pub const fn attack_fraction(&self) -> AttackFraction {
        self.fraction
    }

    /// Margin decay step.
    #[must_use]
    pub const fn margin(&self) -> MarginState {
        self.margin
    }

    /// Player-level Fever state.
    #[must_use]
    pub const fn fever(&self) -> FeverState {
        self.fever
    }

    /// Mutable Fever state, for the safety point's transition step.
    pub const fn fever_mut(&mut self) -> &mut FeverState {
        &mut self.fever
    }

    /// The open Fever session, if any.
    #[must_use]
    pub const fn session(&self) -> Option<&FeverSession> {
        self.session.as_ref()
    }

    /// Links committed in the chain currently being resolved.
    #[must_use]
    pub const fn chain_count(&self) -> u8 {
        self.chain_total_links
    }

    /// The per-level puzzle bags, which are rules state.
    #[must_use]
    pub const fn bags(&self) -> &PuzzleBags {
        &self.bags
    }

    /// Switches to the Fever channel and loads the session's first puzzle.
    ///
    /// The normal channel is frozen rather than cleared: it keeps its board and
    /// its queue, and only stops receiving drops.
    pub fn enter_fever(&mut self, spec: &LockedMatchSpec, level_bonus: i8) -> bool {
        let book = &spec.fever.puzzles;
        // The session opens at the profile's baseline level; entering on an
        // all clear starts the first puzzle that much higher.
        let level = (i32::from(spec.fever.initial_level) + i32::from(level_bonus)).clamp(
            i32::from(spec.fever.min_level),
            i32::from(spec.fever.max_level),
        ) as u8;

        let bags = std::mem::take(&mut self.bags);
        let Some(session) = FeverSession::open(book, level, bags, &mut self.fever_rng) else {
            return false;
        };
        let Some(puzzle) = puzzle_by_id(book, session.current_puzzle_id()) else {
            return false;
        };
        load_puzzle(&mut self.boards[FEVER_CHANNEL], puzzle);
        self.session = Some(session);
        self.fever.enter();
        self.active_channel = FEVER_CHANNEL;
        self.active = None;
        self.split = None;
        self.nuisance_fall = None;
        self.resolution = None;
        true
    }

    /// Applies the level ladder and loads the next puzzle.
    ///
    /// Returns whether the Fever board actually switched to a new puzzle, so
    /// the caller can raise [`MatchEvent::FeverPuzzleAdvanced`] exactly when
    /// the board changed under the player.
    ///
    /// [`MatchEvent::FeverPuzzleAdvanced`]: crate::match_state::MatchEvent::FeverPuzzleAdvanced
    pub fn advance_fever_puzzle(
        &mut self,
        spec: &LockedMatchSpec,
        achieved: u8,
        all_clear: bool,
    ) -> bool {
        let Some(session) = &mut self.session else {
            return false;
        };
        let next = next_target_level(
            spec.fever.level_ladder,
            session.target_level(),
            achieved,
            all_clear,
            spec.fever.min_level,
            spec.fever.max_level,
        );
        let advanced = session
            .advance(&spec.fever.puzzles, next, &mut self.fever_rng)
            .is_some();
        // `RuleLibrary::partial` guarantees every level in the profile's
        // domain has book coverage, so `advance` failing here means the
        // Fever board is about to silently keep showing a stale puzzle.
        debug_assert!(
            advanced,
            "Fever puzzle book has no puzzle for level {next}; the board will not switch"
        );
        if advanced
            && let Some(puzzle) = puzzle_by_id(&spec.fever.puzzles, session.current_puzzle_id())
        {
            let puzzle = puzzle.clone();
            load_puzzle(&mut self.boards[FEVER_CHANNEL], &puzzle);
        }
        advanced
    }

    /// Leaves Fever, merging the frozen queue back into the normal channel.
    ///
    /// The Fever board, its session and its column order are all discarded;
    /// only the exact queue count survives.
    pub fn exit_fever(&mut self, spec: &LockedMatchSpec) {
        let merged = self.pending[FEVER_CHANNEL];
        self.pending[FEVER_CHANNEL] = 0;
        crate::nuisance::enqueue(
            &mut self.pending[NORMAL_CHANNEL],
            merged,
            spec.nuisance.queue_limit,
        );
        self.drop_state[FEVER_CHANNEL] = NuisanceDropState::default();
        self.boards[FEVER_CHANNEL] = Board::with_geometry(spec.board_geometry);
        if let Some(session) = self.session.take() {
            self.bags = session.bags().clone();
        }
        self.fever.exit();
        self.active_channel = NORMAL_CHANNEL;
        self.active = None;
        self.split = None;
        self.nuisance_fall = None;
        self.resolution = None;
    }

    /// Loads the preset puzzle a normal-board all clear awards.
    pub fn load_all_clear_puzzle(&mut self, spec: &LockedMatchSpec) {
        if let Some(puzzle) = puzzle_by_id(&spec.fever.puzzles, &spec.fever.all_clear_puzzle_id) {
            let puzzle = puzzle.clone();
            load_puzzle(&mut self.boards[NORMAL_CHANNEL], &puzzle);
        }
    }

    /// Whether this player has lost the round.
    #[must_use]
    pub const fn is_defeated(&self) -> bool {
        self.defeated
    }

    /// The settlement formed on this tick, if any.
    #[must_use]
    pub const fn settlement(&self) -> Option<&PlayerSettlement> {
        self.settlement.as_ref()
    }

    /// Takes the settlement so the safety point consumes it exactly once.
    pub fn take_settlement(&mut self) -> Option<PlayerSettlement> {
        self.settlement.take()
    }

    /// Advances this player by one rules tick.
    ///
    /// Returns `true` when the group locked on this tick. Cross-player effects
    /// are never applied here: the tick only produces facts.
    pub fn step(
        &mut self,
        spec: &LockedMatchSpec,
        actions: PlayerActions,
        round_tick: u64,
    ) -> bool {
        self.margin.advance_to(&spec.margin, round_tick);
        // The Fever clock does not pause for settlement animation.
        self.fever.tick();

        if self.defeated {
            return false;
        }

        if let Some(split) = &mut self.split {
            split.tick(&mut self.boards[self.active_channel]);
            if split.is_complete() {
                self.split = None;
                self.begin_resolution(spec);
            }
            return false;
        }

        // A batch released at the last safety point is still on its way down.
        // The next group is not supplied until it lands, so this player has
        // nothing to control and no input to consume meanwhile.
        if let Some(fall) = &mut self.nuisance_fall {
            fall.tick();
            if fall.is_complete() {
                let landed = self.nuisance_fall.take().expect("the fall was just ticked");
                self.boards[self.active_channel] = landed.into_target_board();
            }
            return false;
        }

        if self.resolution.is_some() {
            self.advance_resolution(spec, round_tick);
            return false;
        }

        let Some(group) = &mut self.active else {
            return false;
        };
        let outcome = self.control.step(
            group,
            &mut self.boards[self.active_channel],
            actions,
            ControlRules {
                timing: &spec.drop,
                fall_ticks_by_distance: &spec.resolution.gravity_ticks_by_distance,
                color_count: spec.color_count,
            },
        );
        let ControlOutcome::Locked { split, .. } = outcome else {
            return false;
        };
        // Soft-drop points are display only on this profile, so they are added
        // to the score without ever reaching the attack conversion.
        if spec.offense.soft_drop_points_per_cell > 0 {
            let points =
                u64::from(self.control.soft_drop_cells()) * spec.offense.soft_drop_points_per_cell;
            if spec.offense.soft_drop_counts_toward_attack {
                self.score.add_chain_score(points);
            } else {
                self.score.add_soft_drop_score(points);
            }
        }
        self.active = None;
        match split {
            Some(split) => self.split = Some(split),
            None => self.begin_resolution(spec),
        }
        true
    }

    fn begin_resolution(&mut self, spec: &LockedMatchSpec) {
        self.pending_attack = 0;
        let resolution = ResolutionState::new(
            self.boards[self.active_channel].clone(),
            spec.resolution.clone(),
        );
        // The rules layer may compute the whole chain up front; only the
        // committed phases are ever exposed. Knowing the length here is what
        // lets a Fever reward land on the last link's preview tick.
        let mut preview = resolution.clone();
        self.chain_total_links = preview.settle().links.len() as u8;
        self.resolution = Some(resolution);
    }

    /// Advances resolution, converting each link on the tick it commits.
    ///
    /// Scoring happens at `ClearCommit` rather than at settlement because the
    /// design publishes a link's attack on that boundary; carrying the
    /// remainder across links makes the per-link total identical to converting
    /// the whole chain at once.
    fn advance_resolution(&mut self, spec: &LockedMatchSpec, round_tick: u64) {
        let Some(resolution) = &mut self.resolution else {
            return;
        };
        resolution.tick();
        let mode = if self.fever.active() {
            BoardMode::Fever
        } else {
            BoardMode::Normal
        };
        // Inside Fever a held reward is paid when the last link starts its
        // clear animation, not at settlement.
        if let ResolutionPhase::ClearPreview {
            facts,
            elapsed_ticks,
            ..
        } = resolution.phase()
            && *elapsed_ticks == 1
            && facts.chain_index == self.chain_total_links
            && self.fever.active()
        {
            self.fever.release_deferred_reward();
        }
        if let ResolutionPhase::ClearCommit { facts } = resolution.phase() {
            let link_score =
                spec.scoring
                    .score_link(facts, &spec.chain_power[self.chain_power_slot], mode);
            self.score.add_chain_score(link_score);
            let target = self
                .margin
                .target_points(&spec.margin.target_points_by_step)
                .unwrap_or(spec.target_points);
            let converted = self.fraction.convert(link_score, target);
            self.pending_attack = self
                .pending_attack
                .saturating_add(u32::try_from(converted).unwrap_or(u32::MAX));
        }
        if let ResolutionPhase::Settlement(report) = resolution.phase() {
            let report = report.clone();
            self.boards[self.active_channel] = resolution.board().clone();
            self.resolution = None;
            self.settlement = Some(PlayerSettlement {
                attack: self.pending_attack,
                report,
            });
            self.pending_attack = 0;
        }
        let _ = round_tick;
    }

    /// Offsets an incoming attack against this player's own queues.
    ///
    /// The active channel is consumed first; the frozen channel can still be
    /// offset even though it does not receive drops.
    pub fn offset(&mut self, attack: u32) -> crate::nuisance::OffsetFacts {
        let other = (self.active_channel + 1) % CHANNELS;
        let (active, rest) = if self.active_channel < other {
            let (head, tail) = self.pending.split_at_mut(other);
            (&mut head[self.active_channel], &mut tail[0])
        } else {
            let (head, tail) = self.pending.split_at_mut(self.active_channel);
            (&mut tail[0], &mut head[other])
        };
        crate::nuisance::offset_attack(attack, active, rest)
    }

    /// Adds nuisance sent by the opponent to the active channel.
    pub fn receive(&mut self, amount: u32, limit: u32) -> u32 {
        crate::nuisance::enqueue(&mut self.pending[self.active_channel], amount, limit)
    }

    /// Releases a nuisance batch when this turn triggered no chain.
    ///
    /// The batch enters at the top of its columns and starts falling; it is on
    /// the board from here on, but the board does not reach its resting shape
    /// until the fall completes.
    pub fn release(
        &mut self,
        spec: &LockedMatchSpec,
        chain_triggered: bool,
    ) -> Option<NuisanceLanding> {
        let channel = self.active_channel;
        let landing = release_nuisance(
            &mut self.boards[channel],
            &mut self.pending[channel],
            &mut self.drop_state[channel],
            spec.nuisance,
            chain_triggered,
        )?;
        self.nuisance_fall = NuisanceFall::plan(
            &mut self.boards[channel],
            &spec.resolution.gravity_ticks_by_distance,
        );
        Some(landing)
    }

    /// Supplies the next group, marking the player defeated when it cannot.
    ///
    /// This is the only moment a defeat is decided, on either channel.
    pub fn supply_next(&mut self, spec: &LockedMatchSpec) -> bool {
        match spawn_group(
            &self.boards[self.active_channel],
            &mut self.stream,
            spec.board_geometry,
            spec.color_count,
            &mut self.color_rng,
        ) {
            Ok(group) => {
                self.active = Some(group);
                self.control = ControlState::new();
                true
            }
            Err(_) => {
                self.defeated = true;
                false
            }
        }
    }

    /// Whether the player is waiting for a new group.
    ///
    /// This is what the safety point's supply step reads, so released nuisance
    /// still in the air holds the next group back until it has landed.
    #[must_use]
    pub const fn needs_group(&self) -> bool {
        self.active.is_none()
            && self.split.is_none()
            && self.nuisance_fall.is_none()
            && self.resolution.is_none()
            && !self.defeated
    }

    /// The nuisance stream, reserved for garbage-column allocation.
    #[must_use]
    pub const fn nuisance_rng(&self) -> &MatchRng {
        &self.nuisance_rng
    }
}

impl crate::digest::Digestible for PlayerSettlement {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.u32(self.attack);
        self.report.digest_into(writer);
    }
}

impl crate::digest::Digestible for PlayerBattleState {
    fn digest_into(&self, writer: &mut crate::digest::DigestWriter) {
        writer.len(self.active_channel);
        for board in &self.boards {
            board.digest_into(writer);
        }
        for pending in &self.pending {
            writer.u32(*pending);
        }
        for state in &self.drop_state {
            state.digest_into(writer);
        }
        self.stream.digest_into(writer);
        match &self.active {
            Some(group) => {
                writer.u8(1);
                group.digest_into(writer);
            }
            None => writer.u8(0),
        }
        self.control.digest_into(writer);
        match &self.split {
            Some(split) => {
                writer.u8(1);
                split.digest_into(writer);
            }
            None => writer.u8(0),
        }
        match &self.nuisance_fall {
            Some(fall) => {
                writer.u8(1);
                fall.digest_into(writer);
            }
            None => writer.u8(0),
        }
        match &self.resolution {
            Some(resolution) => {
                writer.u8(1);
                resolution.digest_into(writer);
            }
            None => writer.u8(0),
        }
        self.score.digest_into(writer);
        self.fraction.digest_into(writer);
        self.margin.digest_into(writer);
        self.fever.digest_into(writer);
        match &self.session {
            Some(session) => {
                writer.u8(1);
                session.digest_into(writer);
            }
            None => writer.u8(0),
        }
        self.bags.digest_into(writer);
        self.color_rng.digest_into(writer);
        self.nuisance_rng.digest_into(writer);
        self.fever_rng.digest_into(writer);
        writer.u32(self.pending_attack);
        match &self.settlement {
            Some(settlement) => {
                writer.u8(1);
                settlement.digest_into(writer);
            }
            None => writer.u8(0),
        }
        writer.bool(self.defeated);
        writer.len(self.chain_power_slot);
        writer.u8(self.chain_total_links);
    }
}
