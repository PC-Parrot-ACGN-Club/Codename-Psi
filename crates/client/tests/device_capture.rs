//! Real keyboard and gamepad state reaching the sampler through the plugin.
//!
//! These drive `ButtonInput`/`Gamepad` directly rather than the sampler's
//! press/release helpers, so they cover the binding resolution and default
//! bindings that the pure sampling tests deliberately bypass.

mod common;

use bevy::ecs::message::Messages;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use client::app_state::AppState;
use client::input::{STICK_THRESHOLD, UIAction, UIActionEvent};
use client::simulation::RuleState;
use common::{advance_to, controlled_app, run_fixed_tick};
use game_core::input::{GameAction, PlayerActions};

fn press_key(app: &mut App, code: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(code);
}

fn release_key(app: &mut App, code: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(code);
}

/// Device capture runs in `Update`; the rules path runs in `FixedUpdate`, so a
/// realistic frame needs both.
fn capture_and_tick(app: &mut App) {
    app.update();
    run_fixed_tick(app);
}

/// Sample one fixed tick and report the canonical actions for a player slot.
fn tick_actions(app: &mut App, player: usize) -> PlayerActions {
    capture_and_tick(app);
    app.world()
        .resource::<client::simulation::CurrentTickInputs>()
        .inputs
        .player(player)
        .expect("the slot must exist")
}

fn spawn_gamepad(app: &mut App) -> Entity {
    app.world_mut().spawn(Gamepad::default()).id()
}

// docs/test/game-infrastructure.md TC-062
#[test]
fn default_bindings_let_the_keyboard_drive_every_rule_action() {
    let mut app = controlled_app();
    advance_to(&mut app, AppState::Match);

    // P1 defaults: A/D are fixed directions, S/W/K/J the configurable four.
    for (code, action) in [
        (KeyCode::KeyA, GameAction::Left),
        (KeyCode::KeyD, GameAction::Right),
        (KeyCode::KeyS, GameAction::SoftDrop),
        (KeyCode::KeyW, GameAction::HardDrop),
        (KeyCode::KeyK, GameAction::RotateClockwise),
        (KeyCode::KeyJ, GameAction::RotateCounterClockwise),
    ] {
        press_key(&mut app, code);
        let actions = tick_actions(&mut app, 0);
        assert!(
            actions.contains(action),
            "{code:?} must produce {action:?} with default bindings, got {actions:?}"
        );
        release_key(&mut app, code);
        capture_and_tick(&mut app);
    }
}

// docs/test/game-infrastructure.md TC-062
#[test]
fn default_bindings_let_a_gamepad_drive_every_rule_action() {
    let mut app = controlled_app();
    let pad = spawn_gamepad(&mut app);
    advance_to(&mut app, AppState::Match);

    // Directions are fixed to the D-pad; the configurable four default to
    // DPadDown/DPadUp/South/West.
    for (button, action) in [
        (GamepadButton::DPadLeft, GameAction::Left),
        (GamepadButton::DPadRight, GameAction::Right),
        (GamepadButton::DPadDown, GameAction::SoftDrop),
        (GamepadButton::DPadUp, GameAction::HardDrop),
        (GamepadButton::South, GameAction::RotateClockwise),
        (GamepadButton::West, GameAction::RotateCounterClockwise),
    ] {
        app.world_mut()
            .entity_mut(pad)
            .get_mut::<Gamepad>()
            .expect("the pad stays spawned")
            .digital_mut()
            .press(button);

        let actions = tick_actions(&mut app, 0);
        assert!(
            actions.contains(action),
            "{button:?} must produce {action:?} with default bindings, got {actions:?}"
        );

        app.world_mut()
            .entity_mut(pad)
            .get_mut::<Gamepad>()
            .expect("the pad stays spawned")
            .digital_mut()
            .release(button);
        capture_and_tick(&mut app);
    }
}

// docs/test/game-infrastructure.md TC-062
#[test]
fn the_two_local_players_use_their_own_default_keys() {
    let mut app = controlled_app();
    advance_to(&mut app, AppState::Match);

    press_key(&mut app, KeyCode::KeyS);
    press_key(&mut app, KeyCode::ArrowDown);
    capture_and_tick(&mut app);

    let inputs = app
        .world()
        .resource::<client::simulation::CurrentTickInputs>()
        .inputs;
    assert!(
        inputs
            .player(0)
            .expect("P1 slot")
            .contains(GameAction::SoftDrop),
        "P1 soft drop is KeyS"
    );
    assert!(
        inputs
            .player(1)
            .expect("P2 slot")
            .contains(GameAction::SoftDrop),
        "P2 soft drop is ArrowDown"
    );
}

// docs/test/game-infrastructure.md TC-064
#[test]
fn holding_a_direction_produces_one_action_per_tick_without_repeat() {
    let mut app = controlled_app();
    advance_to(&mut app, AppState::Match);
    press_key(&mut app, KeyCode::KeyA);

    for tick in 0..5 {
        let actions = tick_actions(&mut app, 0);
        assert_eq!(
            actions,
            PlayerActions::from_action(GameAction::Left),
            "tick {tick} must carry exactly Left, with no auto-repeat inflation"
        );
    }
}

// docs/test/game-infrastructure.md TC-063
#[test]
fn the_left_stick_only_reports_a_direction_past_the_threshold() {
    for (value, expected) in [
        (STICK_THRESHOLD - 0.1, false),
        (STICK_THRESHOLD, false),
        (STICK_THRESHOLD + 0.1, true),
    ] {
        let mut app = controlled_app();
        let pad = spawn_gamepad(&mut app);
        advance_to(&mut app, AppState::Match);

        app.world_mut()
            .entity_mut(pad)
            .get_mut::<Gamepad>()
            .expect("the pad stays spawned")
            .analog_mut()
            .set(GamepadAxis::LeftStickX, value);

        let actions = tick_actions(&mut app, 0);
        assert_eq!(
            actions.contains(GameAction::Right),
            expected,
            "stick x={value} must {} produce Right",
            if expected { "" } else { "not" }
        );
    }
}

// docs/test/game-infrastructure.md TC-063
#[test]
fn a_horizontal_stick_hold_does_not_leak_into_a_vertical_direction() {
    let mut app = controlled_app();
    let pad = spawn_gamepad(&mut app);
    advance_to(&mut app, AppState::Match);

    app.world_mut()
        .entity_mut(pad)
        .get_mut::<Gamepad>()
        .expect("the pad stays spawned")
        .analog_mut()
        .set(GamepadAxis::LeftStickX, -1.0);

    let actions = tick_actions(&mut app, 0);
    assert_eq!(actions, PlayerActions::from_action(GameAction::Left));
}

// docs/test/game-infrastructure.md TC-065
#[test]
fn escape_proposes_a_pause_only_inside_match() {
    let mut app = controlled_app();
    advance_to(&mut app, AppState::MainMenu);

    press_key(&mut app, KeyCode::Escape);
    app.update();
    assert_eq!(
        *app.world().resource::<State<AppState>>().get(),
        AppState::MainMenu,
        "Escape outside Match means Back, not Pause"
    );
    release_key(&mut app, KeyCode::Escape);

    advance_to(&mut app, AppState::Match);
    let before = *app.world().resource::<RuleState>();

    press_key(&mut app, KeyCode::Escape);
    app.update();
    app.update();

    assert_eq!(
        *app.world().resource::<State<AppState>>().get(),
        AppState::Paused,
        "Escape inside Match proposes a pause"
    );
    assert_eq!(
        *app.world().resource::<RuleState>(),
        before,
        "pausing must not advance or reset the rule state"
    );
}

/// Drain the UI actions emitted during the frames run so far.
fn drain_ui_actions(app: &mut App) -> Vec<UIActionEvent> {
    let messages = app.world().resource::<Messages<UIActionEvent>>();
    let mut cursor = messages.get_cursor();
    cursor.read(messages).copied().collect()
}

// docs/test/game-infrastructure.md TC-052
#[test]
fn menu_context_turns_fixed_directions_into_ui_actions() {
    let mut app = controlled_app();
    advance_to(&mut app, AppState::MainMenu);

    press_key(&mut app, KeyCode::KeyA);
    press_key(&mut app, KeyCode::Space);
    app.update();

    let emitted = drain_ui_actions(&mut app);
    assert!(
        emitted.contains(&UIActionEvent {
            player: 0,
            action: UIAction::Left,
        }),
        "KeyA is P1's fixed Left in a menu, got {emitted:?}"
    );
    assert!(
        emitted.contains(&UIActionEvent {
            player: 0,
            action: UIAction::Confirm,
        }),
        "Space is P1's fixed Confirm, got {emitted:?}"
    );
}

// docs/test/game-infrastructure.md TC-052
#[test]
fn the_same_physical_direction_does_not_produce_ui_actions_inside_match() {
    let mut app = controlled_app();
    advance_to(&mut app, AppState::Match);

    press_key(&mut app, KeyCode::KeyA);
    app.update();

    assert!(
        drain_ui_actions(&mut app).is_empty(),
        "in Match the direction is rules input only, never a focus move"
    );
    assert!(
        tick_actions(&mut app, 0).contains(GameAction::Left),
        "the same key still drives the rules action"
    );
}

// docs/test/game-infrastructure.md TC-064
#[test]
fn holding_a_menu_direction_moves_focus_once() {
    let mut app = controlled_app();
    advance_to(&mut app, AppState::MainMenu);

    press_key(&mut app, KeyCode::KeyA);
    for _ in 0..5 {
        app.update();
    }

    let moves = drain_ui_actions(&mut app)
        .into_iter()
        .filter(|event| event.action == UIAction::Left)
        .count();
    assert_eq!(
        moves, 1,
        "a held direction must not auto-repeat focus moves"
    );
}

// docs/test/game-infrastructure.md TC-052
#[test]
fn the_two_local_players_emit_their_own_ui_actions() {
    let mut app = controlled_app();
    advance_to(&mut app, AppState::MainMenu);

    press_key(&mut app, KeyCode::Space);
    press_key(&mut app, KeyCode::Enter);
    app.update();

    let emitted = drain_ui_actions(&mut app);
    for player in 0..2 {
        assert!(
            emitted.contains(&UIActionEvent {
                player,
                action: UIAction::Confirm,
            }),
            "player {player} has its own fixed Confirm key, got {emitted:?}"
        );
    }
}
