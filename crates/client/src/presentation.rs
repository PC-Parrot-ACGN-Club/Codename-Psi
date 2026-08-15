//! Pure rule-to-presentation projection and headless runtime seams.

use std::collections::HashSet;

use game_core::{
    board::{Board, Cell, Coord},
    config::CharacterId,
    drop_stream::PendingHand,
    falling::FallingGroup,
    match_spec::LockedMatchSpec,
    match_state::{MatchEvent, MatchOutcome, MatchPhase, MatchStepReport},
    view::{MatchView, ResolutionView},
};

use crate::{app_state::AppState, settings::AnimationIntensity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerPresentationSnapshot {
    pub board: Board,
    pub active_drop: Option<FallingGroup>,
    pub next_drops: Vec<PendingHand>,
    pub drop_set_id: CharacterId,
    pub score: u64,
    pub pending_garbage: u32,
    pub fever_garbage: u32,
    pub fever_gauge: u8,
    pub fever_time_ticks: u32,
    pub fever_target: Option<u8>,
    pub fever_state: bool,
    pub overflow_risk: bool,
    pub chain_count: u8,
    pub resolution: Option<ResolutionView>,
    /// Cells the loaded Fever puzzle put on the board.
    ///
    /// Drawn as translucent starting markers so the preset chain is not read as
    /// balls the player stacked. The list is the puzzle's own cells, so a
    /// preset ball that has been cleared simply leaves an empty cell behind.
    pub preset_cells: Vec<Coord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Momentum {
    pub net_attack: [u32; 2],
    pub pressure: [u32; 2],
    pub advantage_side: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPresentationSnapshot {
    pub match_tick: u64,
    pub players: [PlayerPresentationSnapshot; 2],
    pub round: u32,
    pub wins: [u8; 2],
    pub phase: MatchPhase,
    pub result: Option<MatchOutcome>,
    pub momentum: Momentum,
    /// How much disposable motion this snapshot may be drawn with.
    ///
    /// Carried by the snapshot rather than read from settings at draw time so
    /// that rebuilding the screen from one snapshot stays sufficient, which is
    /// the property the whole resident layer is built on.
    pub effects: PresentationEffects,
}

#[must_use]
pub fn build_snapshot(
    view: Option<&MatchView>,
    report: Option<&MatchStepReport>,
    spec: &LockedMatchSpec,
    intensity: AnimationIntensity,
) -> Option<MatchPresentationSnapshot> {
    let view = view?;
    let players = std::array::from_fn(|slot| {
        let player = &view.players[slot];
        let geometry = player.board.geometry();
        let risk = Coord::new(geometry.spawn_column(), geometry.hidden_rows())
            .is_some_and(|coord| player.board.get(coord) != Cell::Empty);
        PlayerPresentationSnapshot {
            board: player.board.clone(),
            active_drop: player.active_group,
            next_drops: player.next.clone(),
            drop_set_id: player.drop_set_id.clone(),
            score: player.score,
            pending_garbage: player.pending[0],
            fever_garbage: player.pending[1],
            fever_gauge: player.fever_gauge,
            fever_time_ticks: player.fever_time_ticks,
            fever_target: player.fever_target,
            fever_state: player.in_fever,
            overflow_risk: risk,
            chain_count: player.chain_count,
            resolution: player.resolution.clone(),
            preset_cells: player
                .fever_puzzle_id
                .as_deref()
                .and_then(|id| game_core::fever::puzzle_by_id(&spec.fever.puzzles, id))
                .map(|puzzle| {
                    puzzle
                        .cells
                        .iter()
                        .filter_map(|cell| Coord::new(cell.x, cell.y))
                        .collect()
                })
                .unwrap_or_default(),
        }
    });
    let mut net_attack = [0_u32; 2];
    if let Some(report) = report {
        for event in &report.events {
            if let MatchEvent::AttackArbitrated { slot, sent, .. } = event
                && let Some(value) = net_attack.get_mut(*slot)
            {
                *value = value.saturating_add(*sent);
            }
        }
    }
    let pressure = std::array::from_fn(|slot| {
        players[slot]
            .pending_garbage
            .saturating_add(players[slot].fever_garbage)
            .saturating_add(if players[slot].overflow_risk { 100 } else { 0 })
    });
    let strength: [i64; 2] = std::array::from_fn(|slot| {
        i64::from(net_attack[slot]) + i64::from(pressure[1 - slot]) - i64::from(pressure[slot])
            + if players[slot].fever_state { 25 } else { 0 }
    });
    let advantage_side = match strength[0].cmp(&strength[1]) {
        std::cmp::Ordering::Greater => Some(0),
        std::cmp::Ordering::Less => Some(1),
        std::cmp::Ordering::Equal => None,
    };
    Some(MatchPresentationSnapshot {
        match_tick: view.match_tick,
        players,
        round: view.round,
        wins: view.wins,
        phase: view.phase,
        result: match view.phase {
            MatchPhase::Completed(outcome) => Some(outcome),
            _ => None,
        },
        momentum: Momentum {
            net_attack,
            pressure,
            advantage_side,
        },
        effects: PresentationEffects::of(intensity),
    })
}

/// The pose a participant's portrait holds this frame.
///
/// One pose per participant per frame, chosen from facts the snapshot already
/// carries plus the line the last few ticks left. The order below is the
/// priority: a decided match outranks Fever, Fever outranks what just left the
/// board, and pressure is what shows when nothing else is happening.
#[must_use]
pub fn portrait_pose(
    snapshot: &MatchPresentationSnapshot,
    lines: &FeedbackLines,
    slot: usize,
) -> crate::character_presentation::PoseKind {
    use crate::character_presentation::PoseKind;

    let player = &snapshot.players[slot];
    let decided = match snapshot.phase {
        MatchPhase::Completed(outcome) => Some(outcome.winner == slot),
        MatchPhase::RoundOutro { outcome, .. } => match outcome {
            game_core::match_state::RoundOutcome::Decided(winner) => Some(winner == slot),
            game_core::match_state::RoundOutcome::Draw => None,
        },
        _ => None,
    };
    if let Some(won) = decided {
        return if won {
            PoseKind::Winning
        } else {
            PoseKind::Losing
        };
    }
    if player.fever_state {
        return PoseKind::Fever;
    }
    match lines.line(slot, snapshot.match_tick) {
        Some(FeedbackLine::Attack(_) | FeedbackLine::AllClear) => return PoseKind::Attacking,
        Some(FeedbackLine::Offset(_)) => return PoseKind::Offsetting,
        None => {}
    }
    if player.overflow_risk {
        return PoseKind::Strained;
    }
    if player.pending_garbage + player.fever_garbage > 0 {
        return PoseKind::Defending;
    }
    PoseKind::Idle
}

/// Icon units a nuisance queue is read in, heaviest first.
///
/// A greedy walk therefore spends the heaviest symbols first, and a queue too
/// large to spell out loses only its lightest icons.
pub const NUISANCE_UNITS: [u32; 7] = [1440, 720, 360, 180, 30, 6, 1];

/// How many icons one queue has room for.
pub const NUISANCE_ICON_SLOTS: usize = 8;

/// The tier units standing for `count`, heaviest first.
///
/// The exact count is shown beside the icons, so truncating at
/// [`NUISANCE_ICON_SLOTS`] hides no information: it keeps the panel a fixed
/// size while the number stays exact.
#[must_use]
pub fn nuisance_icons(count: u32) -> Vec<u32> {
    let mut remaining = count;
    let mut icons = Vec::new();
    for unit in NUISANCE_UNITS {
        while remaining >= unit && icons.len() < NUISANCE_ICON_SLOTS {
            icons.push(unit);
            remaining -= unit;
        }
    }
    icons
}

/// One line of transient text a tick's facts put over a player's board.
///
/// The exact numbers are the rules' own, not a projection of them: the queue
/// panel already shows what is pending, and this says what just moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackLine {
    /// An attack reached the opponent.
    Attack(u32),
    /// An attack was fully cancelled against queued nuisance.
    Offset(u32),
    /// The visible board ended empty.
    AllClear,
}

impl FeedbackLine {
    /// The localization key naming this fact.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Attack(_) => "match.feedback.attack",
            Self::Offset(_) => "match.feedback.offset",
            Self::AllClear => "match.feedback.all_clear",
        }
    }

    /// The line as shown: the fact's name, then its exact count.
    #[must_use]
    pub fn text(self, localization: &crate::i18n::Localization) -> String {
        let name = localization.text(self.key());
        match self {
            Self::Attack(amount) | Self::Offset(amount) => format!("{name} {amount}"),
            Self::AllClear => name,
        }
    }
}

/// How long one line stays up, in rule ticks.
///
/// Counted in ticks rather than seconds so the line is up for the same span of
/// play at any frame rate, and so a paused match holds what it was showing.
pub const FEEDBACK_TICKS: u64 = 90;

/// The line each participant is currently showing, and since when.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackLines {
    lines: [Option<(FeedbackLine, u64)>; 2],
}

impl FeedbackLines {
    /// Records what one tick's facts put on screen.
    ///
    /// A tick with nothing to say leaves the previous line alone: lines expire
    /// on their own clock, so an uneventful tick is not an instruction to clear
    /// the screen.
    pub fn observe(&mut self, report: &MatchStepReport) {
        for (slot, line) in self.lines.iter_mut().enumerate() {
            if let Some(fact) = tick_line(report, slot) {
                *line = Some((fact, report.match_tick));
            }
        }
    }

    /// The line a slot still shows at `match_tick`, if it has not expired.
    #[must_use]
    pub fn line(&self, slot: usize, match_tick: u64) -> Option<FeedbackLine> {
        let (line, since) = (*self.lines.get(slot)?)?;
        (match_tick.saturating_sub(since) < FEEDBACK_TICKS).then_some(line)
    }
}

/// The one line a tick's facts leave for a slot.
///
/// An all clear outranks the attack it produced: the attack is already legible
/// in the opponent's queue, and the empty board is the rarer fact. Between the
/// remaining two, what reached the opponent outranks what was cancelled, so a
/// partially offset attack still reads as pressure sent.
fn tick_line(report: &MatchStepReport, slot: usize) -> Option<FeedbackLine> {
    let mut all_clear = false;
    let mut arbitrated = None;
    for event in &report.events {
        match event {
            MatchEvent::ChainSettled {
                slot: event_slot,
                all_clear: cleared,
                ..
            } if *event_slot == slot => all_clear |= cleared,
            MatchEvent::AttackArbitrated {
                slot: event_slot,
                offset,
                sent,
            } if *event_slot == slot => arbitrated = Some((*offset, *sent)),
            _ => {}
        }
    }
    if all_clear {
        return Some(FeedbackLine::AllClear);
    }
    match arbitrated? {
        (_, sent) if sent > 0 => Some(FeedbackLine::Attack(sent)),
        (offset, _) if offset > 0 => Some(FeedbackLine::Offset(offset)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentationEventId {
    pub match_tick: u64,
    pub ordinal: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationEffects {
    pub particle_density: u8,
    pub interpolate: bool,
}

impl PresentationEffects {
    /// The disposable motion one intensity setting allows.
    ///
    /// `Reduced` drops to a single cue per fact and stops interpolating, so a
    /// phase shows its start and its end and nothing between. Neither setting
    /// touches `duration_ticks` or any other rule timing.
    #[must_use]
    pub const fn of(intensity: AnimationIntensity) -> Self {
        match intensity {
            AnimationIntensity::Full => Self {
                particle_density: 100,
                interpolate: true,
            },
            AnimationIntensity::Reduced => Self {
                particle_density: 1,
                interpolate: false,
            },
        }
    }

    /// Marks one cleared ball leaves.
    ///
    /// Zero is not the absence of feedback: the caller still leaves one mark
    /// for the whole clear, which is the single hint per fact the reduced
    /// setting keeps. Spending the density per cell is what makes a large
    /// clear look large at full intensity.
    #[must_use]
    pub const fn marks_per_cell(self) -> usize {
        self.particle_density as usize * MARKS_PER_CELL / 100
    }
}

/// Marks one cleared ball leaves at full intensity.
pub const MARKS_PER_CELL: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationEvent {
    pub id: PresentationEventId,
    pub fact: MatchEvent,
    pub effects: PresentationEffects,
}

#[must_use]
pub fn publish_events(
    report: &MatchStepReport,
    intensity: AnimationIntensity,
) -> Vec<PresentationEvent> {
    let effects = PresentationEffects::of(intensity);
    report
        .events
        .iter()
        .enumerate()
        .map(|(ordinal, fact)| PresentationEvent {
            id: PresentationEventId {
                match_tick: report.match_tick,
                ordinal: ordinal as u16,
            },
            fact: fact.clone(),
            effects,
        })
        .collect()
}

#[derive(Debug, Default)]
pub struct PresentationEventConsumer {
    performed: HashSet<PresentationEventId>,
}

impl PresentationEventConsumer {
    pub fn consume(&mut self, event: &PresentationEvent) -> bool {
        self.performed.insert(event.id)
    }

    #[must_use]
    pub fn performed_count(&self) -> usize {
        self.performed.len()
    }
}

/// The cue one rule fact plays.
///
/// One kind per fact, fixed before there is any sample to play: R1 ships no
/// audio (see [PRD §4.2]), so what has to be right now is the interface and
/// the tick each cue is asked for, not the sound.
///
/// [PRD §4.2]: ../../../docs/PRD.md
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioCue {
    Lock,
    Chain,
    AllClear,
    Attack,
    Offset,
    NuisanceLanded,
    FeverEntered,
    FeverExited,
    Defeat,
    RoundEnded,
    MatchEnded,
}

impl AudioCue {
    /// The cue a fact plays.
    #[must_use]
    pub const fn of(fact: &MatchEvent) -> Self {
        match fact {
            MatchEvent::GroupLocked(_) => Self::Lock,
            MatchEvent::ChainSettled {
                all_clear: true, ..
            } => Self::AllClear,
            MatchEvent::ChainSettled { .. } => Self::Chain,
            MatchEvent::AttackArbitrated { sent, .. } if *sent > 0 => Self::Attack,
            MatchEvent::AttackArbitrated { .. } => Self::Offset,
            MatchEvent::NuisanceDropped { .. } => Self::NuisanceLanded,
            MatchEvent::FeverEntered(_) => Self::FeverEntered,
            MatchEvent::FeverExited(_) => Self::FeverExited,
            MatchEvent::PlayerDefeated(_) => Self::Defeat,
            MatchEvent::RoundEnded(_) => Self::RoundEnded,
            MatchEvent::MatchEnded(_) => Self::MatchEnded,
        }
    }
}

/// The short vibration patterns the presentation contract names.
///
/// Only four kinds of fact vibrate; everything else is heard and seen but not
/// felt, so a busy tick does not turn into a continuous buzz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VibrationPattern {
    Chain,
    NuisanceLanded,
    FeverEntered,
    RoundDecided,
}

impl VibrationPattern {
    /// The pattern a cue vibrates with, if it vibrates at all.
    #[must_use]
    pub const fn of(cue: AudioCue) -> Option<Self> {
        match cue {
            AudioCue::Chain | AudioCue::AllClear => Some(Self::Chain),
            AudioCue::NuisanceLanded => Some(Self::NuisanceLanded),
            AudioCue::FeverEntered => Some(Self::FeverEntered),
            AudioCue::RoundEnded | AudioCue::MatchEnded => Some(Self::RoundDecided),
            AudioCue::Lock
            | AudioCue::Attack
            | AudioCue::Offset
            | AudioCue::FeverExited
            | AudioCue::Defeat => None,
        }
    }
}

/// The two volume settings an effect cue passes through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioGains {
    pub master: f32,
    pub sfx: f32,
}

impl AudioGains {
    /// Both sliders at full.
    pub const FULL: Self = Self {
        master: 1.0,
        sfx: 1.0,
    };

    /// The gain one effect cue plays at.
    ///
    /// The two sliders multiply, and either at zero silences the cue without
    /// cancelling it: the request still happens, so what was triggered and when
    /// stays observable however the sliders are set.
    #[must_use]
    pub fn effect(self) -> f32 {
        (self.master * self.sfx).clamp(0.0, 1.0)
    }
}

/// One cue asked for by one confirmed fact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CueRequest {
    pub id: PresentationEventId,
    pub cue: AudioCue,
    /// The participant the fact belongs to, or `None` for a match-level fact.
    pub slot: Option<usize>,
    /// Gain after both volume settings; zero when there is no audio output.
    pub gain: f32,
    /// Pattern to play on this participant's pad, once the setting and the
    /// connected pads have had their say.
    pub vibration: Option<VibrationPattern>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPresentationFrame {
    pub snapshot: MatchPresentationSnapshot,
}

impl MatchPresentationFrame {
    #[must_use]
    pub fn from_snapshot(snapshot: &MatchPresentationSnapshot) -> Self {
        Self {
            snapshot: snapshot.clone(),
        }
    }
}

#[derive(Debug, Default)]
pub struct PresentationRuntime {
    offered: Option<MatchPresentationSnapshot>,
    frame: Option<MatchPresentationFrame>,
}

impl PresentationRuntime {
    pub fn clear_resident_entities(&mut self) {
        self.frame = None;
    }

    pub fn sample(&mut self, snapshot: MatchPresentationSnapshot) {
        self.frame = Some(MatchPresentationFrame::from_snapshot(&snapshot));
        self.offered = None;
    }

    pub fn offer(&mut self, snapshot: MatchPresentationSnapshot) {
        if self
            .offered
            .as_ref()
            .is_none_or(|current| current.match_tick <= snapshot.match_tick)
        {
            self.offered = Some(snapshot);
        }
    }

    pub fn render_latest(&mut self) {
        if let Some(snapshot) = self.offered.take() {
            self.sample(snapshot);
        }
    }

    #[must_use]
    pub const fn frame(&self) -> Option<&MatchPresentationFrame> {
        self.frame.as_ref()
    }
}

#[derive(Debug, Default)]
pub struct EntityLifecycle {
    next_id: u64,
    page: Option<(AppState, u64)>,
    match_entity: Option<u64>,
}

impl EntityLifecycle {
    fn allocate(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn enter(&mut self, state: AppState) {
        self.page = match state {
            AppState::Boot | AppState::Match => None,
            _ => {
                let id = self.allocate();
                Some((state, id))
            }
        };
    }

    pub fn spawn_match_entities(&mut self) {
        if self.match_entity.is_none() {
            self.match_entity = Some(self.allocate());
        }
    }

    #[must_use]
    pub fn page_entity(&self) -> Option<u64> {
        self.page.map(|(_, id)| id)
    }

    #[must_use]
    pub const fn match_entity(&self) -> Option<u64> {
        self.match_entity
    }
}

pub struct VirtualCanvas;

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasLayout {
    pub design_size: (f32, f32),
    pub ui_scale: f32,
    pub world_scale: f32,
    pub letterbox: (f32, f32),
}

impl CanvasLayout {
    #[must_use]
    pub fn anchor(&self, name: &str) -> Option<(f32, f32)> {
        match name {
            "p1_board" => Some((420.0, 540.0)),
            "p2_board" => Some((1500.0, 540.0)),
            _ => None,
        }
    }
}

impl VirtualCanvas {
    #[must_use]
    pub fn layout(width: f32, height: f32) -> CanvasLayout {
        let scale = (width / 1920.0).min(height / 1080.0);
        CanvasLayout {
            design_size: (1920.0, 1080.0),
            ui_scale: scale,
            world_scale: scale,
            letterbox: (
                (width - 1920.0 * scale).max(0.0) / 2.0,
                (height - 1080.0 * scale).max(0.0) / 2.0,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackBudget {
    pub transient_entities: usize,
    pub concurrent_cues: usize,
}

impl Default for FeedbackBudget {
    fn default() -> Self {
        Self {
            transient_entities: 64,
            concurrent_cues: 8,
        }
    }
}

pub struct FeedbackRuntime {
    audio: AudioAvailability,
    gamepads: usize,
    budget: FeedbackBudget,
    seen: HashSet<PresentationEventId>,
    audio_requests: usize,
    vibration_requests: usize,
    transient_entities: usize,
    concurrent_cues: usize,
    merged_batches: usize,
    diagnostics: Vec<String>,
}

impl FeedbackRuntime {
    #[must_use]
    pub fn new(audio: AudioAvailability, gamepads: usize, budget: FeedbackBudget) -> Self {
        Self {
            audio,
            gamepads,
            budget,
            seen: HashSet::new(),
            audio_requests: 0,
            vibration_requests: 0,
            transient_entities: 0,
            concurrent_cues: 0,
            merged_batches: 0,
            diagnostics: Vec::new(),
        }
    }

    /// How many local pads are connected, which is what vibration reaches.
    pub const fn set_gamepads(&mut self, gamepads: usize) {
        self.gamepads = gamepads;
    }

    /// Turns confirmed facts into cue requests, once each.
    ///
    /// The requests are returned rather than played here: R1 has no samples,
    /// and returning them is what makes the cue kinds and their ticks
    /// observable without an audio device.
    pub fn consume(
        &mut self,
        events: &[PresentationEvent],
        gains: AudioGains,
        vibration_enabled: bool,
    ) -> Vec<CueRequest> {
        let fresh: Vec<_> = events
            .iter()
            .filter(|event| self.seen.insert(event.id))
            .collect();
        if fresh.is_empty() {
            return Vec::new();
        }
        let batches = if fresh.len() > 1 {
            self.merged_batches += 1;
            1
        } else {
            1
        };
        self.transient_entities = batches.min(self.budget.transient_entities);
        self.concurrent_cues = batches.min(self.budget.concurrent_cues);
        match self.audio {
            AudioAvailability::Available => self.audio_requests += self.concurrent_cues,
            AudioAvailability::Unavailable => {
                if self.diagnostics.is_empty() {
                    self.diagnostics.push("audio output unavailable".into());
                }
                self.concurrent_cues = 0;
            }
        }
        if self.gamepads > 0 {
            self.vibration_requests += batches.min(self.gamepads);
        }

        let gain = match self.audio {
            AudioAvailability::Available => gains.effect(),
            AudioAvailability::Unavailable => 0.0,
        };
        fresh
            .into_iter()
            .map(|event| {
                let cue = AudioCue::of(&event.fact);
                let slot = event.fact.participant();
                // A pad only buzzes for the player holding it, so a fact with
                // no participant -- a round or match ending -- reaches nobody's
                // pad through this path.
                let reaches_a_pad =
                    slot.is_some_and(|slot| slot < self.gamepads) && vibration_enabled;
                CueRequest {
                    id: event.id,
                    cue,
                    slot,
                    gain,
                    vibration: reaches_a_pad.then(|| VibrationPattern::of(cue)).flatten(),
                }
            })
            .collect()
    }

    #[must_use]
    pub const fn audio_requests(&self) -> usize {
        self.audio_requests
    }
    #[must_use]
    pub const fn vibration_requests(&self) -> usize {
        self.vibration_requests
    }
    #[must_use]
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
    #[must_use]
    pub const fn live_transient_entities(&self) -> usize {
        self.transient_entities
    }
    /// Whether an audio device is present.
    #[must_use]
    pub const fn audio(&self) -> AudioAvailability {
        self.audio
    }
    #[must_use]
    pub const fn concurrent_cues(&self) -> usize {
        self.concurrent_cues
    }
    #[must_use]
    pub const fn merged_batches(&self) -> usize {
        self.merged_batches
    }
}
