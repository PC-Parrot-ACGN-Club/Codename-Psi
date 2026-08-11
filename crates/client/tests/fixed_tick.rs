//! 60 Hz configuration, `Input -> Rules` ordering, single consumption, and determinism.

mod common;

use std::time::Duration;

use bevy::prelude::*;
use client::app_state::AppState;
use client::simulation::{CurrentTickInputs, FIXED_HZ, FixedStage, RuleState, SimulationProbe};
use common::{advance_to, controlled_app, current_state, install_sampler, press, release};
use game_core::input::GameAction;

/// Three ticks whose inputs differ, so each tick's product is identifiable.
const TICK_MARKERS: [GameAction; 3] = [
    GameAction::SoftDrop,
    GameAction::HardDrop,
    GameAction::RotateClockwise,
];

fn match_app() -> App {
    let mut app = controlled_app();
    install_sampler(&mut app, 1);
    advance_to(&mut app, AppState::Match);
    app
}

/// Drive one controlled fixed tick whose input is exactly `marker`.
fn tick_with(app: &mut App, marker: GameAction) {
    press(app, 0, marker);
    app.world_mut().run_schedule(FixedUpdate);
    release(app, 0, marker);
}

// docs/test/game-infrastructure.md TC-042
#[test]
fn the_fixed_schedule_is_configured_for_sixty_hertz() {
    let app = match_app();

    let timestep = app.world().resource::<Time<Fixed>>().timestep();

    assert_eq!(FIXED_HZ, 60.0);
    assert_eq!(
        timestep,
        Duration::from_secs_f64(1.0 / 60.0),
        "the configured timestep is read directly, not derived from accumulated time"
    );
}

// docs/test/game-infrastructure.md TC-043
#[test]
fn every_fixed_tick_runs_input_strictly_before_rules() {
    let mut app = match_app();

    for marker in TICK_MARKERS {
        tick_with(&mut app, marker);
    }

    let probe = app.world().resource::<SimulationProbe>();
    assert_eq!(
        probe.stages,
        vec![
            FixedStage::Input,
            FixedStage::Rules,
            FixedStage::Input,
            FixedStage::Rules,
            FixedStage::Input,
            FixedStage::Rules,
        ]
    );
    for (tick, consumed) in probe.consumed_inputs.iter().enumerate() {
        assert!(
            consumed
                .player(0)
                .expect("slot 0 is active")
                .contains(TICK_MARKERS[tick]),
            "tick {tick} rules must read the input produced in the same tick"
        );
    }
}

// docs/test/game-infrastructure.md TC-044
#[test]
fn each_fixed_tick_forms_and_consumes_its_tick_inputs_exactly_once() {
    let mut app = match_app();

    for marker in TICK_MARKERS {
        tick_with(&mut app, marker);
    }

    let probe = app.world().resource::<SimulationProbe>();
    assert_eq!(probe.produced, 3);
    assert_eq!(probe.consumed, 3);
    assert_eq!(probe.consumed_inputs.len(), 3);

    for marker in TICK_MARKERS {
        let uses = probe
            .consumed_inputs
            .iter()
            .filter(|inputs| {
                inputs
                    .player(0)
                    .is_some_and(|actions| actions.contains(marker))
            })
            .count();
        assert_eq!(uses, 1, "{marker:?} must be consumed exactly once");
    }
    assert!(
        app.world().resource::<CurrentTickInputs>().is_consumed(),
        "the last tick's inputs must be marked consumed"
    );
}

// docs/test/game-infrastructure.md TC-044
#[test]
fn a_second_rules_pass_in_the_same_tick_does_not_consume_again() {
    let mut app = match_app();
    tick_with(&mut app, GameAction::SoftDrop);

    // Re-run only the rules stage; the tick's inputs are already consumed.
    let consumed_before = app.world().resource::<SimulationProbe>().consumed;
    app.world_mut()
        .run_system_cached(client::simulation::advance_rules)
        .expect("rules system runs");

    assert_eq!(
        app.world().resource::<SimulationProbe>().consumed,
        consumed_before,
        "tick inputs must never be consumed twice"
    );
}

/// Run the same six-tick input sequence, interleaving `updates_per_tick` ordinary
/// `Update` runs that must not advance the rules path.
fn run_six_tick_sequence(updates_per_tick: usize) -> (RuleState, SimulationProbe) {
    let mut app = match_app();
    let sequence = [
        GameAction::SoftDrop,
        GameAction::HardDrop,
        GameAction::RotateClockwise,
        GameAction::SoftDrop,
        GameAction::RotateCounterClockwise,
        GameAction::HardDrop,
    ];

    for marker in sequence {
        for _ in 0..updates_per_tick {
            app.update();
        }
        tick_with(&mut app, marker);
    }

    let rules = *app.world().resource::<RuleState>();
    let probe = std::mem::take(app.world_mut().resource_mut::<SimulationProbe>().as_mut());
    (rules, probe)
}

// docs/test/game-infrastructure.md TC-045
#[test]
fn identical_inputs_produce_identical_rule_results_under_different_update_counts() {
    let (sparse_rules, sparse_probe) = run_six_tick_sequence(1);
    let (dense_rules, dense_probe) = run_six_tick_sequence(5);

    assert_eq!(sparse_rules.tick, 6, "only fixed ticks advance the rules");
    assert_eq!(dense_rules.tick, 6, "extra updates must not advance rules");
    assert_eq!(
        sparse_rules, dense_rules,
        "the same initial state and quantized inputs must yield the same rule state"
    );
    assert_eq!(sparse_probe.consumed_inputs, dense_probe.consumed_inputs);
    assert_eq!(sparse_probe.produced, dense_probe.produced);
}

// docs/test/game-infrastructure.md TC-045
#[test]
fn ordinary_updates_alone_never_advance_the_rule_state() {
    let mut app = match_app();
    let before = *app.world().resource::<RuleState>();

    for _ in 0..10 {
        app.update();
    }

    assert_eq!(*app.world().resource::<RuleState>(), before);
    assert_eq!(app.world().resource::<SimulationProbe>().consumed, 0);
    assert_eq!(current_state(&app), AppState::Match);
}
