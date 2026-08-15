//! Read-only projection of a match, for presentation and for AI.
//!
//! The view carries exactly what a human player can see. It has no random
//! state and no hands beyond the visible NEXT queue, so anything reading it —
//! including an AI — is limited to the same information a person has.

use crate::{
    board::{Board, Coord},
    config::CharacterId,
    drop_stream::PendingHand,
    falling::FallingGroup,
    match_spec::PARTICIPANT_SLOTS,
    match_state::{MatchPhase, MatchState},
    player::{CHANNELS, PlayerBattleState},
    resolution::{GravityMove, ResolutionPhase},
};

/// Stable resolution stage exposed without the resolver's private state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStage {
    Idle,
    ClearPreview,
    ClearCommit,
    Gravity,
    ScanNext,
    Settlement,
}

/// Read-only progress needed to reconstruct a resolving field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionView {
    pub stage: ResolutionStage,
    pub chain_index: u8,
    pub elapsed_ticks: u16,
    pub duration_ticks: u16,
    pub clear_cells: Vec<Coord>,
    pub gravity_moves: Vec<GravityMove>,
}

impl ResolutionView {
    /// Released nuisance falling, projected as the gravity stage it is.
    ///
    /// A falling batch is not a chain link, but it is the same fall over the
    /// same table, so presentation reads it through one shape instead of two.
    fn of_nuisance_fall(fall: &crate::nuisance::NuisanceFall) -> Self {
        Self {
            stage: ResolutionStage::Gravity,
            chain_index: 0,
            elapsed_ticks: fall.elapsed_ticks(),
            duration_ticks: fall.duration_ticks(),
            clear_cells: Vec::new(),
            gravity_moves: fall.moves().to_vec(),
        }
    }

    fn of(phase: &ResolutionPhase) -> Self {
        match phase {
            ResolutionPhase::Idle => Self {
                stage: ResolutionStage::Idle,
                chain_index: 0,
                elapsed_ticks: 0,
                duration_ticks: 0,
                clear_cells: Vec::new(),
                gravity_moves: Vec::new(),
            },
            ResolutionPhase::ClearPreview {
                facts,
                elapsed_ticks,
                duration_ticks,
            } => Self {
                stage: ResolutionStage::ClearPreview,
                chain_index: facts.chain_index,
                elapsed_ticks: *elapsed_ticks,
                duration_ticks: *duration_ticks,
                clear_cells: facts
                    .cleared_colored_coords
                    .iter()
                    .chain(&facts.cleared_nuisance_coords)
                    .copied()
                    .collect(),
                gravity_moves: Vec::new(),
            },
            ResolutionPhase::ClearCommit { facts } => Self {
                stage: ResolutionStage::ClearCommit,
                chain_index: facts.chain_index,
                elapsed_ticks: 0,
                duration_ticks: 0,
                clear_cells: facts
                    .cleared_colored_coords
                    .iter()
                    .chain(&facts.cleared_nuisance_coords)
                    .copied()
                    .collect(),
                gravity_moves: Vec::new(),
            },
            ResolutionPhase::Gravity {
                moves,
                elapsed_ticks,
                duration_ticks,
                ..
            } => Self {
                stage: ResolutionStage::Gravity,
                chain_index: 0,
                elapsed_ticks: *elapsed_ticks,
                duration_ticks: *duration_ticks,
                clear_cells: Vec::new(),
                gravity_moves: moves.clone(),
            },
            ResolutionPhase::ScanNext { next_chain_index } => Self {
                stage: ResolutionStage::ScanNext,
                chain_index: *next_chain_index,
                elapsed_ticks: 0,
                duration_ticks: 0,
                clear_cells: Vec::new(),
                gravity_moves: Vec::new(),
            },
            ResolutionPhase::Settlement(report) => Self {
                stage: ResolutionStage::Settlement,
                chain_index: report.links.len() as u8,
                elapsed_ticks: 0,
                duration_ticks: 0,
                clear_cells: Vec::new(),
                gravity_moves: Vec::new(),
            },
        }
    }
}

/// What one participant can see of their own side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerView {
    /// Board of the active channel.
    pub board: Board,
    /// Board of the frozen channel, which is public information.
    pub frozen_board: Board,
    /// Which channel is active.
    pub active_channel: usize,
    /// Group under control, if any.
    pub active_group: Option<FallingGroup>,
    /// Hands already drawn into NEXT. Nothing beyond it is visible.
    pub next: Vec<PendingHand>,
    /// Selected character, which identifies the frozen drop set on the client.
    pub drop_set_id: CharacterId,
    /// Exact pending nuisance per channel.
    pub pending: [u32; CHANNELS],
    /// Filled gauge cells.
    pub fever_gauge: u8,
    /// Gauge cells needed to enter.
    pub fever_capacity: u8,
    /// Remaining Fever time in ticks.
    pub fever_time_ticks: u32,
    /// Whether the Fever channel is active.
    pub in_fever: bool,
    /// Target chain level of the current Fever puzzle.
    pub fever_target: Option<u8>,
    /// Displayed score.
    pub score: u64,
    /// Turn the active group belongs to.
    pub turn_id: u32,
    /// Whether this participant has lost the round.
    pub defeated: bool,
    /// Links committed in the current resolution.
    pub chain_count: u8,
    /// Current resolution phase and progress, if a group is settling.
    pub resolution: Option<ResolutionView>,
}

impl PlayerView {
    fn of(player: &PlayerBattleState, drop_set_id: CharacterId) -> Self {
        let other = (player.active_channel() + 1) % CHANNELS;
        Self {
            board: player.board().clone(),
            frozen_board: player
                .channel_board(other)
                .cloned()
                .unwrap_or_else(|| player.board().clone()),
            active_channel: player.active_channel(),
            active_group: player.active_group().copied(),
            next: player.stream().queued().collect(),
            drop_set_id,
            pending: [player.pending(0), player.pending(1)],
            fever_gauge: player.fever().gauge(),
            fever_capacity: player.fever().capacity(),
            fever_time_ticks: player.fever().time_ticks(),
            in_fever: player.fever().active(),
            fever_target: player.session().map(|session| session.target_level()),
            score: player.score().displayed(),
            turn_id: player.active_group().map_or(0, FallingGroup::turn_id),
            defeated: player.is_defeated(),
            chain_count: player.chain_count(),
            resolution: player
                .resolution()
                .map(|resolution| ResolutionView::of(resolution.phase()))
                .or_else(|| player.nuisance_fall().map(ResolutionView::of_nuisance_fall)),
        }
    }
}

/// Read-only projection of the whole match.
///
/// This is a projection, not a parallel state: nothing written here can reach
/// the rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchView {
    /// Ticks consumed since the match began.
    pub match_tick: u64,
    /// Zero-based current round number.
    pub round: u32,
    /// Lifecycle phase.
    pub phase: MatchPhase,
    /// Rounds won per participant.
    pub wins: [u8; PARTICIPANT_SLOTS],
    /// Both participants' visible state.
    pub players: [PlayerView; PARTICIPANT_SLOTS],
}

impl MatchState {
    /// Projects the current state into the read model.
    #[must_use]
    pub fn view(&self) -> MatchView {
        MatchView {
            match_tick: self.match_tick(),
            round: self.round_index(),
            phase: self.phase(),
            wins: self.wins(),
            players: std::array::from_fn(|slot| {
                PlayerView::of(
                    self.round().player(slot).expect("every slot exists"),
                    self.spec().characters[slot].clone(),
                )
            }),
        }
    }

    /// Projects one participant's visible state.
    #[must_use]
    pub fn player_view(&self, slot: usize) -> Option<PlayerView> {
        Some(PlayerView::of(
            self.round().player(slot)?,
            self.spec().characters.get(slot)?.clone(),
        ))
    }
}
