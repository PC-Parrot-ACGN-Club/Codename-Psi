//! Top-level client state and its single transition entry point.

use bevy::prelude::*;

#[derive(Debug, Default)]
pub struct AppStatePlugin;

impl Plugin for AppStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<AppTransitionRequests>()
            .init_resource::<AppTransitionDiagnostics>()
            .add_systems(Update, arbitrate_transitions);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
pub enum AppState {
    #[default]
    Boot,
    MainMenu,
    ModeSelect,
    CharacterSelect,
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
    PauseRequested,
    ResumeRequested,
    MatchCompleted,
    ReturnToMainMenu,
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
            | (AppState::MainMenu, AppState::ModeSelect)
            | (AppState::ModeSelect, AppState::CharacterSelect)
            | (AppState::CharacterSelect, AppState::Match)
            | (AppState::Match, AppState::Paused | AppState::Result)
            | (AppState::Paused, AppState::Match)
            | (AppState::Result, AppState::MainMenu)
    )
}

pub fn arbitrate_transitions(
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut requests: ResMut<AppTransitionRequests>,
    mut diagnostics: ResMut<AppTransitionDiagnostics>,
) {
    let current = *state.get();
    let mut valid = Vec::new();

    for request in requests.pending.drain(..) {
        if request.target == current {
            continue;
        }
        if is_valid_transition(current, request.target) {
            if !valid.contains(&request) {
                valid.push(request);
            }
        } else {
            diagnostics.0.push(AppTransitionDiagnostic::InvalidEdge {
                from: current,
                to: request.target,
            });
        }
    }

    let selected = valid.iter().copied().find(|request| {
        request.cause == AppTransitionCause::MatchCompleted && request.target == AppState::Result
    });
    let unique_targets: Vec<_> = valid.iter().map(|request| request.target).collect();
    let selected = selected.or_else(|| {
        unique_targets
            .first()
            .copied()
            .filter(|target| unique_targets.iter().all(|other| other == target))
            .and_then(|target| {
                valid
                    .iter()
                    .copied()
                    .find(|request| request.target == target)
            })
    });

    if let Some(request) = selected {
        next_state.set(request.target);
    } else if unique_targets.len() > 1 {
        diagnostics
            .0
            .push(AppTransitionDiagnostic::ConflictingTargets(unique_targets));
    }
}
