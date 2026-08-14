//! Read-only projection of a match, for presentation and for AI.
//!
//! The view carries exactly what a human player can see. It has no random
//! state and no hands beyond the visible NEXT queue, so anything reading it —
//! including an AI — is limited to the same information a person has.

use crate::{
    board::Board,
    drop_stream::PendingHand,
    falling::FallingGroup,
    match_spec::PARTICIPANT_SLOTS,
    match_state::{MatchPhase, MatchState},
    player::{CHANNELS, PlayerBattleState},
};

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
    /// Displayed score.
    pub score: u64,
    /// Turn the active group belongs to.
    pub turn_id: u32,
    /// Whether this participant has lost the round.
    pub defeated: bool,
}

impl PlayerView {
    fn of(player: &PlayerBattleState) -> Self {
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
            pending: [player.pending(0), player.pending(1)],
            fever_gauge: player.fever().gauge(),
            fever_capacity: player.fever().capacity(),
            fever_time_ticks: player.fever().time_ticks(),
            in_fever: player.fever().active(),
            score: player.score().displayed(),
            turn_id: player.active_group().map_or(0, FallingGroup::turn_id),
            defeated: player.is_defeated(),
        }
    }
}

/// Read-only projection of the whole match.
///
/// This is a projection, not a parallel state: nothing written here can reach
/// the rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchView {
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
            phase: self.phase(),
            wins: self.wins(),
            players: std::array::from_fn(|slot| {
                PlayerView::of(self.round().player(slot).expect("every slot exists"))
            }),
        }
    }

    /// Projects one participant's visible state.
    #[must_use]
    pub fn player_view(&self, slot: usize) -> Option<PlayerView> {
        Some(PlayerView::of(self.round().player(slot)?))
    }
}
