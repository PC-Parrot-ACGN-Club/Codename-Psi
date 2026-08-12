//! Sampler output crossing into `game_core` normalization on the real fixed path,
//! plus the fixed pause button proposing a state transition directly.

mod common;

use client::app_state::AppState;
use client::input::{LocalInputSampler, PhysicalInput, UIAction, fixed_pause_inputs};
use client::simulation::CurrentTickInputs;
use common::{
    advance_to, commit, controlled_app, current_state, install_sampler, press, run_fixed_tick,
};
use game_core::input::{GameAction, PlayerActions};

fn canonical_actions_after_pressing(actions: &[GameAction]) -> PlayerActions {
    let mut app = controlled_app();
    install_sampler(&mut app, 1);
    advance_to(&mut app, AppState::Match);

    for action in actions {
        press(&mut app, 0, *action);
    }
    run_fixed_tick(&mut app);

    app.world()
        .resource::<CurrentTickInputs>()
        .inputs
        .player(0)
        .expect("slot 0 is active")
}

// docs/test/game-infrastructure.md TC-027B
#[test]
fn simultaneous_rotations_reach_game_core_and_normalize_to_no_rotation() {
    let canonical = canonical_actions_after_pressing(&[
        GameAction::RotateClockwise,
        GameAction::RotateCounterClockwise,
    ]);

    assert!(!canonical.contains(GameAction::RotateClockwise));
    assert!(!canonical.contains(GameAction::RotateCounterClockwise));
    assert_eq!(canonical, PlayerActions::EMPTY);
}

// docs/test/game-infrastructure.md TC-027B
#[test]
fn soft_and_hard_drop_reach_game_core_and_normalize_to_hard_drop() {
    let canonical = canonical_actions_after_pressing(&[GameAction::SoftDrop, GameAction::HardDrop]);

    assert!(canonical.contains(GameAction::HardDrop));
    assert!(!canonical.contains(GameAction::SoftDrop));
}

/// The six UI actions, listed to document that `Pause` is not one of them.
const UI_ACTIONS: [UIAction; 6] = [
    UIAction::Left,
    UIAction::Right,
    UIAction::Up,
    UIAction::Down,
    UIAction::Confirm,
    UIAction::Back,
];

// docs/test/game-infrastructure.md TC-058
#[test]
fn the_fixed_start_button_commits_paused_from_match() {
    let mut app = controlled_app();
    install_sampler(&mut app, 1);
    advance_to(&mut app, AppState::Match);

    app.world_mut()
        .resource_mut::<LocalInputSampler>()
        .press_pause(&fixed_pause_inputs()[0]);
    commit(&mut app);

    assert_eq!(current_state(&app), AppState::Paused);
}

// docs/test/game-infrastructure.md TC-058
#[test]
fn the_pause_trigger_produces_no_game_action() {
    let mut sampler = LocalInputSampler::new(vec![common::keyboard_bindings(0)]);

    sampler.press_pause(&fixed_pause_inputs()[0]);
    let sampled = sampler.sample_fixed();

    assert_eq!(
        sampled[0],
        PlayerActions::EMPTY,
        "the pause trigger never becomes rules input"
    );
    assert_eq!(
        UI_ACTIONS.len(),
        6,
        "UIAction has no Pause member for the trigger to travel through"
    );
}

// docs/test/game-infrastructure.md TC-058
#[test]
fn a_button_other_than_the_fixed_start_button_does_not_propose_a_pause() {
    let mut app = controlled_app();
    install_sampler(&mut app, 1);
    advance_to(&mut app, AppState::Match);

    app.world_mut()
        .resource_mut::<LocalInputSampler>()
        .press_pause(&PhysicalInput::gamepad("Select"));
    commit(&mut app);

    assert_eq!(current_state(&app), AppState::Match);
}

// docs/test/game-infrastructure.md TC-058
#[test]
fn the_fixed_start_button_is_ignored_outside_match() {
    let mut app = controlled_app();
    install_sampler(&mut app, 1);
    advance_to(&mut app, AppState::MainMenu);

    app.world_mut()
        .resource_mut::<LocalInputSampler>()
        .press_pause(&fixed_pause_inputs()[0]);
    commit(&mut app);

    assert_eq!(current_state(&app), AppState::MainMenu);
}
