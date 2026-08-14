//! Top-level client state and its single transition entry point.

use bevy::prelude::*;

#[derive(Debug, Default)]
pub struct AppStatePlugin;

impl Plugin for AppStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<AppTransitionRequests>()
            .init_resource::<AppTransitionDiagnostics>()
            .insert_resource(SettingsOrigin(AppState::MainMenu))
            .configure_sets(
                Update,
                (AppTransitionSet::Request, AppTransitionSet::Arbitrate).chain(),
            )
            .add_systems(
                Update,
                arbitrate_transitions.in_set(AppTransitionSet::Arbitrate),
            );
    }
}

/// Ordering contract for the single transition entry point.
///
/// Requesters join [`AppTransitionSet::Request`] instead of ordering
/// themselves against the arbiter by name, so a new requester cannot forget
/// the `.before(..)` and silently miss its submission cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum AppTransitionSet {
    /// Where components submit an [`AppTransitionRequest`].
    Request,
    /// Where the arbiter validates, merges and commits exactly one target.
    Arbitrate,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
pub enum AppState {
    #[default]
    Boot,
    MainMenu,
    ModeSelect,
    CharacterSelect,
    Settings,
    Match,
    Paused,
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppTransitionCause {
    BootstrapReady,
    StartGame,
    ModeConfirmed,
    CharacterConfirmed,
    BackRequested,
    SettingsOpened,
    SettingsClosed,
    MatchStartRequested,
    PauseRequested,
    ResumeRequested,
    RestartRequested,
    MatchCompleted,
    RematchRequested,
    MatchAbandoned,
    ReturnToMainMenu,
}

/// State from which the settings overlay was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct SettingsOrigin(pub AppState);

/// The transition selected by the arbiter for the next state commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct CommittedTransition {
    pub from: AppState,
    pub to: AppState,
    pub cause: AppTransitionCause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppTransitionRequest {
    pub target: AppState,
    pub cause: AppTransitionCause,
}

#[derive(Debug, Default, Resource)]
pub struct AppTransitionRequests {
    pub pending: Vec<AppTransitionRequest>,
}

impl AppTransitionRequests {
    pub fn submit(&mut self, target: AppState, cause: AppTransitionCause) {
        self.pending.push(AppTransitionRequest { target, cause });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppTransitionDiagnostic {
    InvalidEdge { from: AppState, to: AppState },
    ConflictingTargets(Vec<AppState>),
}

#[derive(Debug, Default, Resource)]
pub struct AppTransitionDiagnostics(pub Vec<AppTransitionDiagnostic>);

#[must_use]
pub const fn is_valid_transition(from: AppState, to: AppState) -> bool {
    matches!(
        (from, to),
        (AppState::Boot, AppState::MainMenu)
            | (
                AppState::MainMenu,
                AppState::ModeSelect | AppState::Settings
            )
            | (
                AppState::ModeSelect,
                AppState::CharacterSelect | AppState::MainMenu
            )
            | (
                AppState::CharacterSelect,
                AppState::Match | AppState::ModeSelect
            )
            | (AppState::Settings, AppState::MainMenu | AppState::Paused)
            | (AppState::Match, AppState::Paused | AppState::Result)
            | (
                AppState::Paused,
                AppState::Match | AppState::Settings | AppState::MainMenu
            )
            | (AppState::Result, AppState::Match | AppState::MainMenu)
    )
}

/// Declared precedence between causes that can conflict in one cycle.
///
/// Each entry reads "the first cause wins over the second". Conflicts outside
/// this table are rejected rather than resolved by declaration order, so a new
/// state edge cannot silently inherit an arbitrary winner: adding an edge that
/// can conflict means adding its rule here.
const CAUSE_PRECEDENCE: [(AppTransitionCause, AppTransitionCause); 1] = [(
    AppTransitionCause::MatchCompleted,
    AppTransitionCause::PauseRequested,
)];

/// Whether `winner` is declared to take precedence over `loser`.
fn takes_precedence(winner: AppTransitionCause, loser: AppTransitionCause) -> bool {
    CAUSE_PRECEDENCE
        .iter()
        .any(|(high, low)| *high == winner && *low == loser)
}

pub fn arbitrate_transitions(
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut requests: ResMut<AppTransitionRequests>,
    mut diagnostics: ResMut<AppTransitionDiagnostics>,
    mut commands: Commands,
) {
    let current = *state.get();
    let mut valid: Vec<AppTransitionRequest> = Vec::new();

    for request in requests.pending.drain(..) {
        // A same-state request ends here: it is a no-op, not an invalid edge.
        if request.target == current {
            continue;
        }
        if !is_valid_transition(current, request.target) {
            let diagnostic = AppTransitionDiagnostic::InvalidEdge {
                from: current,
                to: request.target,
            };
            warn!("rejected transition request: {diagnostic:?}");
            diagnostics.0.push(diagnostic);
            continue;
        }
        // Requests for the same target merge into one transition.
        if !valid.iter().any(|kept| kept.target == request.target) {
            valid.push(request);
        }
    }

    let [first, ..] = valid.as_slice() else {
        return;
    };
    if valid.len() == 1 {
        next_state.set(first.target);
        commands.insert_resource(CommittedTransition {
            from: current,
            to: first.target,
            cause: first.cause,
        });
        return;
    }

    // Distinct targets: commit only when exactly one cause outranks every other.
    let mut winners = valid.iter().filter(|candidate| {
        valid.iter().all(|other| {
            other.target == candidate.target || takes_precedence(candidate.cause, other.cause)
        })
    });

    match (winners.next(), winners.next()) {
        (Some(winner), None) => {
            next_state.set(winner.target);
            commands.insert_resource(CommittedTransition {
                from: current,
                to: winner.target,
                cause: winner.cause,
            });
        }
        _ => {
            let diagnostic = AppTransitionDiagnostic::ConflictingTargets(
                valid.iter().map(|request| request.target).collect(),
            );
            warn!("rejected conflicting transition requests: {diagnostic:?}");
            diagnostics.0.push(diagnostic);
        }
    }
}
