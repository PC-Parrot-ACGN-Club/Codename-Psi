//! State registration, lifecycle firing, request de-duplication, and arbitration.

mod common;

use bevy::prelude::*;
use client::app_state::{AppState, AppTransitionCause, CommittedTransition, SettingsOrigin};
use common::{ALL_STATES, advance_to, commit, current_state, state_only_app, submit};

#[derive(Debug, Default, Resource)]
struct LifecycleLog {
    entered: Vec<AppState>,
    exited: Vec<AppState>,
}

impl LifecycleLog {
    fn entries(&self, state: AppState) -> usize {
        self.entered.iter().filter(|seen| **seen == state).count()
    }

    fn exits(&self, state: AppState) -> usize {
        self.exited.iter().filter(|seen| **seen == state).count()
    }

    fn clear(&mut self) {
        self.entered.clear();
        self.exited.clear();
    }
}

/// Counts how often the run phase gated on `ModeSelect` actually executes.
#[derive(Debug, Default, Resource)]
struct ModeSelectPhaseRuns(usize);

fn observed_app() -> App {
    let mut app = state_only_app();
    app.init_resource::<LifecycleLog>();
    app.init_resource::<ModeSelectPhaseRuns>();

    for state in ALL_STATES {
        app.add_systems(OnEnter(state), move |mut log: ResMut<LifecycleLog>| {
            log.entered.push(state)
        });
        app.add_systems(OnExit(state), move |mut log: ResMut<LifecycleLog>| {
            log.exited.push(state)
        });
    }

    app.add_systems(
        Update,
        (|mut runs: ResMut<ModeSelectPhaseRuns>| runs.0 += 1)
            .run_if(in_state(AppState::ModeSelect)),
    );
    app
}

fn clear_log(app: &mut App) {
    app.world_mut().resource_mut::<LifecycleLog>().clear();
}

// integration-system/application-lifecycle::TC-001
#[test]
fn registering_the_state_machine_starts_in_boot() {
    let mut app = state_only_app();

    assert_eq!(current_state(&app), AppState::Boot);

    app.update();

    assert_eq!(
        current_state(&app),
        AppState::Boot,
        "the first schedule must not leave Boot on its own"
    );
}

// integration-system/application-lifecycle::TC-002
#[test]
fn requesting_the_current_state_is_a_no_op_for_every_state() {
    for state in ALL_STATES {
        let mut app = observed_app();
        advance_to(&mut app, state);
        // Settle the initial state entry so only same-state effects are counted.
        commit(&mut app);
        clear_log(&mut app);

        submit(&mut app, state, AppTransitionCause::ReturnToMainMenu);
        commit(&mut app);

        let log = app.world().resource::<LifecycleLog>();
        assert_eq!(current_state(&app), state, "{state:?} must stay current");
        assert_eq!(
            log.entered.len(),
            0,
            "{state:?} must not re-enter on a same-state request"
        );
        assert_eq!(
            log.exited.len(),
            0,
            "{state:?} must not exit on a same-state request"
        );
    }
}

// integration-system/application-lifecycle::TC-002
#[test]
fn a_same_state_request_leaves_no_invalid_edge_diagnostic() {
    let mut app = observed_app();
    advance_to(&mut app, AppState::MainMenu);

    submit(&mut app, AppState::MainMenu, AppTransitionCause::StartGame);
    commit(&mut app);

    let diagnostics = app
        .world()
        .resource::<client::app_state::AppTransitionDiagnostics>();
    assert!(
        diagnostics.0.is_empty(),
        "a same-state request is a no-op, not an invalid edge: {diagnostics:?}"
    );
}

// integration-system/application-lifecycle::TC-003
#[test]
fn a_committed_transition_fires_one_exit_and_one_enter() {
    let mut app = observed_app();
    advance_to(&mut app, AppState::MainMenu);
    clear_log(&mut app);
    app.world_mut().resource_mut::<ModeSelectPhaseRuns>().0 = 0;

    submit(
        &mut app,
        AppState::ModeSelect,
        AppTransitionCause::StartGame,
    );
    commit(&mut app);

    assert_eq!(current_state(&app), AppState::ModeSelect);
    let log = app.world().resource::<LifecycleLog>();
    assert_eq!(log.exits(AppState::MainMenu), 1);
    assert_eq!(log.entries(AppState::ModeSelect), 1);
    assert!(
        app.world().resource::<ModeSelectPhaseRuns>().0 > 0,
        "the run phase for the entered state must become active"
    );
}

// integration-system/application-lifecycle::TC-004
#[test]
fn two_identical_requests_in_one_cycle_merge_into_one_transition() {
    let mut app = observed_app();
    advance_to(&mut app, AppState::Match);
    clear_log(&mut app);

    submit(
        &mut app,
        AppState::Result,
        AppTransitionCause::MatchCompleted,
    );
    submit(
        &mut app,
        AppState::Result,
        AppTransitionCause::MatchCompleted,
    );
    commit(&mut app);

    assert_eq!(current_state(&app), AppState::Result);
    let log = app.world().resource::<LifecycleLog>();
    assert_eq!(log.exits(AppState::Match), 1);
    assert_eq!(log.entries(AppState::Result), 1);
    assert_eq!(log.entered.len(), 1);
}

// integration-system/application-lifecycle::TC-005
#[test]
fn match_completed_wins_over_pause_requested_when_result_is_submitted_first() {
    let mut app = observed_app();
    advance_to(&mut app, AppState::Match);

    submit(
        &mut app,
        AppState::Result,
        AppTransitionCause::MatchCompleted,
    );
    submit(
        &mut app,
        AppState::Paused,
        AppTransitionCause::PauseRequested,
    );
    commit(&mut app);

    assert_eq!(current_state(&app), AppState::Result);
}

// integration-system/application-lifecycle::TC-005
#[test]
fn match_completed_wins_over_pause_requested_when_pause_is_submitted_first() {
    let mut app = observed_app();
    advance_to(&mut app, AppState::Match);
    clear_log(&mut app);

    submit(
        &mut app,
        AppState::Paused,
        AppTransitionCause::PauseRequested,
    );
    submit(
        &mut app,
        AppState::Result,
        AppTransitionCause::MatchCompleted,
    );
    commit(&mut app);

    assert_eq!(current_state(&app), AppState::Result);
    assert_eq!(
        app.world()
            .resource::<LifecycleLog>()
            .entries(AppState::Paused),
        0,
        "the losing request must never enter Paused"
    );
}

/// The rejection branch is reachable today: `Match` is the one state with two
/// valid targets, so a cause pair outside the precedence table must leave the
/// state untouched rather than let declaration order pick a winner.
#[test]
fn a_conflict_without_declared_precedence_is_rejected() {
    let mut app = observed_app();
    advance_to(&mut app, AppState::Match);

    submit(
        &mut app,
        AppState::Result,
        AppTransitionCause::ReturnToMainMenu,
    );
    submit(
        &mut app,
        AppState::Paused,
        AppTransitionCause::PauseRequested,
    );
    commit(&mut app);

    assert_eq!(
        current_state(&app),
        AppState::Match,
        "an undeclared conflict must not change state"
    );

    let diagnostics = &app
        .world()
        .resource::<client::app_state::AppTransitionDiagnostics>()
        .0;
    assert!(
        diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            client::app_state::AppTransitionDiagnostic::ConflictingTargets(_)
        )),
        "the rejection must be observable: {diagnostics:?}"
    );
}

// integration-system/application-lifecycle::TC-011
#[test]
fn settings_returns_to_whichever_state_opened_it() {
    for origin in [AppState::MainMenu, AppState::Paused] {
        let mut app = observed_app();
        advance_to(&mut app, origin);

        // The page records where it came from before asking to open.
        app.insert_resource(SettingsOrigin(origin));
        submit(
            &mut app,
            AppState::Settings,
            AppTransitionCause::SettingsOpened,
        );
        commit(&mut app);
        assert_eq!(current_state(&app), AppState::Settings);

        let target = app.world().resource::<SettingsOrigin>().0;
        submit(&mut app, target, AppTransitionCause::SettingsClosed);
        commit(&mut app);
        assert_eq!(
            current_state(&app),
            origin,
            "settings opened from {origin:?} must return there"
        );
    }
}

// integration-system/application-lifecycle::TC-012
#[test]
fn the_committed_transition_tells_resume_apart_from_restart_on_the_same_edge() {
    for cause in [
        AppTransitionCause::ResumeRequested,
        AppTransitionCause::RestartRequested,
    ] {
        let mut app = observed_app();
        advance_to(&mut app, AppState::Paused);

        submit(&mut app, AppState::Match, cause);
        commit(&mut app);

        let committed = *app.world().resource::<CommittedTransition>();
        assert_eq!(committed.from, AppState::Paused);
        assert_eq!(committed.to, AppState::Match);
        assert_eq!(
            committed.cause, cause,
            "the same edge must carry the cause that produced it"
        );
    }

    // A rejected request leaves the last committed transition alone.
    let mut app = observed_app();
    advance_to(&mut app, AppState::MainMenu);
    let before = *app.world().resource::<CommittedTransition>();
    submit(
        &mut app,
        AppState::Result,
        AppTransitionCause::MatchCompleted,
    );
    commit(&mut app);
    assert_eq!(current_state(&app), AppState::MainMenu);
    assert_eq!(
        *app.world().resource::<CommittedTransition>(),
        before,
        "an invalid edge must not be recorded as committed"
    );
}
