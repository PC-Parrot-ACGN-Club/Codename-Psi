//! Match-only execution of the fixed rules path and its pause/resume behaviour.

mod common;

use bevy::prelude::*;
use client::app_state::{AppState, AppTransitionCause};
use client::simulation::{RuleState, SimulationProbe};
use common::{
    advance_to, commit, controlled_app, current_state, install_sampler, press, release,
    run_fixed_ticks, submit,
};
use game_core::input::GameAction;

const NON_MATCH_STATES: [AppState; 6] = [
    AppState::Boot,
    AppState::MainMenu,
    AppState::ModeSelect,
    AppState::CharacterSelect,
    AppState::Paused,
    AppState::Result,
];

/// A six-tick input sequence used to compare paused and uninterrupted runs.
const SEQUENCE: [GameAction; 6] = [
    GameAction::SoftDrop,
    GameAction::HardDrop,
    GameAction::RotateClockwise,
    GameAction::SoftDrop,
    GameAction::RotateCounterClockwise,
    GameAction::HardDrop,
];

fn app_in(state: AppState) -> App {
    let mut app = controlled_app();
    install_sampler(&mut app, 1);
    advance_to(&mut app, state);
    app
}

fn tick_with(app: &mut App, marker: GameAction) {
    press(app, 0, marker);
    app.world_mut().run_schedule(FixedUpdate);
    release(app, 0, marker);
}

fn probe_counts(app: &App) -> (u64, u64) {
    let probe = app.world().resource::<SimulationProbe>();
    (probe.produced, probe.consumed)
}

// docs/test/game-infrastructure.md TC-053
#[test]
fn no_non_match_state_runs_the_input_or_rules_stage() {
    for state in NON_MATCH_STATES {
        let mut app = app_in(state);
        assert_eq!(current_state(&app), state);

        run_fixed_ticks(&mut app, 3);

        assert_eq!(
            probe_counts(&app),
            (0, 0),
            "{state:?} must not run the fixed game sets"
        );
    }
}

// docs/test/game-infrastructure.md TC-053
#[test]
fn the_match_state_runs_both_stages_once_per_controlled_tick() {
    let mut app = app_in(AppState::Match);

    run_fixed_ticks(&mut app, 3);

    assert_eq!(probe_counts(&app), (3, 3));
}

// docs/test/game-infrastructure.md TC-054
#[test]
fn entering_paused_stops_the_match_simulation_immediately() {
    let mut app = app_in(AppState::Match);
    run_fixed_ticks(&mut app, 3);
    let before = probe_counts(&app);
    assert_eq!(before, (3, 3));

    submit(
        &mut app,
        AppState::Paused,
        AppTransitionCause::PauseRequested,
    );
    commit(&mut app);
    run_fixed_ticks(&mut app, 3);

    assert_eq!(current_state(&app), AppState::Paused);
    assert_eq!(
        probe_counts(&app),
        before,
        "no further Input/Rules execution may happen once Paused is committed"
    );
}

// docs/test/game-infrastructure.md TC-055
#[test]
fn resuming_continues_the_rule_state_from_before_the_pause() {
    let mut app = app_in(AppState::Match);
    for marker in &SEQUENCE[..3] {
        tick_with(&mut app, *marker);
    }
    let paused_from = *app.world().resource::<RuleState>();
    assert_eq!(paused_from.tick, 3);

    submit(
        &mut app,
        AppState::Paused,
        AppTransitionCause::PauseRequested,
    );
    commit(&mut app);
    run_fixed_ticks(&mut app, 3);

    assert_eq!(
        *app.world().resource::<RuleState>(),
        paused_from,
        "the paused period must neither reset nor jump the rule state"
    );

    submit(
        &mut app,
        AppState::Match,
        AppTransitionCause::ResumeRequested,
    );
    commit(&mut app);
    for marker in &SEQUENCE[3..] {
        tick_with(&mut app, *marker);
    }

    let resumed = *app.world().resource::<RuleState>();
    assert_eq!(
        resumed.tick, 6,
        "the tick count continues accumulating from before the pause"
    );

    let mut reference = app_in(AppState::Match);
    for marker in SEQUENCE {
        tick_with(&mut reference, marker);
    }
    assert_eq!(
        resumed,
        *reference.world().resource::<RuleState>(),
        "resuming continues from state S instead of restarting"
    );
}
