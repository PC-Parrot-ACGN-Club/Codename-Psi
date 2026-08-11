//! Sampler output crossing into `game_core` normalization on the real fixed path.

mod common;

use client::app_state::AppState;
use client::simulation::CurrentTickInputs;
use common::{advance_to, controlled_app, install_sampler, press, run_fixed_tick};
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
