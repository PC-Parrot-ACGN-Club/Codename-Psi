//! Gamepads appearing and disappearing mid-session.
//!
//! A pad that is gone can never report a release, and nothing else will ever
//! contradict what it was holding, so the sampler has to drop that state
//! itself.

mod common;

use bevy::ecs::message::Messages;
use bevy::prelude::*;
use client::app_state::AppState;
use client::input::{GamepadSlots, UIAction, UIActionEvent};
use client::simulation::CurrentTickInputs;
use game_core::input::{GameAction, PlayerActions};

use common::{advance_to, controlled_app, run_fixed_tick};

fn spawn_gamepad(app: &mut App) -> Entity {
    app.world_mut().spawn(Gamepad::default()).id()
}

fn hold(app: &mut App, pad: Entity, button: GamepadButton) {
    app.world_mut()
        .entity_mut(pad)
        .get_mut::<Gamepad>()
        .expect("the pad is spawned")
        .digital_mut()
        .press(button);
}

/// Capture devices and consume one fixed tick, as a real frame would.
fn tick_actions(app: &mut App, player: usize) -> PlayerActions {
    app.update();
    run_fixed_tick(app);
    app.world()
        .resource::<CurrentTickInputs>()
        .inputs
        .player(player)
        .expect("the slot must exist")
}

/// Take the UI actions emitted since the last call.
///
/// This really empties the buffer: a read-only cursor would still see the
/// earlier frames' messages, which are retained for two updates.
fn drain_ui_actions(app: &mut App) -> Vec<UIActionEvent> {
    app.world_mut()
        .resource_mut::<Messages<UIActionEvent>>()
        .drain()
        .collect()
}

fn slot_of(app: &App, pad: Entity) -> Option<usize> {
    app.world().resource::<GamepadSlots>().slot(pad)
}

// integration-system/input-and-fixed-tick::TC-014
#[test]
fn a_disconnect_clears_what_that_pad_was_holding() {
    let mut app = controlled_app();
    let pad = spawn_gamepad(&mut app);
    advance_to(&mut app, AppState::Match);
    hold(&mut app, pad, GamepadButton::DPadLeft);

    assert_eq!(
        tick_actions(&mut app, 0),
        PlayerActions::from_action(GameAction::Left),
        "the held direction reaches the rules layer while the pad is connected"
    );

    app.world_mut().entity_mut(pad).despawn();
    assert_eq!(
        tick_actions(&mut app, 0),
        PlayerActions::EMPTY,
        "unplugging while holding a direction must not leave the piece moving forever"
    );
}

// integration-system/input-and-fixed-tick::TC-014
#[test]
fn an_idle_disconnect_leaves_the_other_player_untouched() {
    let mut app = controlled_app();
    let idle = spawn_gamepad(&mut app);
    let active = spawn_gamepad(&mut app);
    advance_to(&mut app, AppState::Match);
    hold(&mut app, active, GamepadButton::DPadLeft);

    assert_eq!(slot_of(&app, idle), Some(0));
    assert_eq!(slot_of(&app, active), Some(1));
    assert_eq!(
        tick_actions(&mut app, 1),
        PlayerActions::from_action(GameAction::Left)
    );

    app.world_mut().entity_mut(idle).despawn();
    assert_eq!(
        tick_actions(&mut app, 1),
        PlayerActions::from_action(GameAction::Left),
        "losing an idle pad must not disturb the player still holding one"
    );
    assert_eq!(
        app.world()
            .resource::<CurrentTickInputs>()
            .inputs
            .player(0)
            .expect("the slot must exist"),
        PlayerActions::EMPTY,
        "the surviving pad must not slide into the freed slot and drive player 0"
    );
}

// integration-system/input-and-fixed-tick::TC-014
#[test]
fn a_reconnected_pad_can_move_the_focus_again() {
    // The UI edge state is where a disconnect really bites: a stale held entry
    // swallows the next press, because a rising edge is only reported when the
    // source was not already held. Nothing clears it on its own -- while no pad
    // is bound, the menu path has no device to report a release from.
    let mut app = controlled_app();
    let pad = spawn_gamepad(&mut app);
    advance_to(&mut app, AppState::MainMenu);
    hold(&mut app, pad, GamepadButton::DPadLeft);
    app.update();
    assert_eq!(
        drain_ui_actions(&mut app)
            .iter()
            .filter(|event| event.action == UIAction::Left)
            .count(),
        1,
        "holding a direction moves the focus once"
    );

    app.world_mut().entity_mut(pad).despawn();
    app.update();

    let replacement = spawn_gamepad(&mut app);
    hold(&mut app, replacement, GamepadButton::DPadLeft);
    app.update();

    assert_eq!(
        slot_of(&app, replacement),
        Some(0),
        "the freed slot is available again"
    );
    assert_eq!(
        drain_ui_actions(&mut app)
            .iter()
            .filter(|event| event.action == UIAction::Left)
            .count(),
        1,
        "a reconnected pad must be able to move the focus again"
    );
}

// integration-system/input-and-fixed-tick::TC-015
#[test]
fn a_disconnect_does_not_move_another_players_pad() {
    let mut app = controlled_app();
    let first = spawn_gamepad(&mut app);
    let second = spawn_gamepad(&mut app);
    advance_to(&mut app, AppState::Match);
    hold(&mut app, second, GamepadButton::DPadRight);

    assert_eq!(slot_of(&app, first), Some(0));
    assert_eq!(slot_of(&app, second), Some(1));

    // The first pad leaves and a third arrives: query order now differs from
    // arrival order, which is exactly what must not decide the binding.
    app.world_mut().entity_mut(first).despawn();
    let third = spawn_gamepad(&mut app);
    hold(&mut app, third, GamepadButton::DPadLeft);

    assert_eq!(
        tick_actions(&mut app, 1),
        PlayerActions::from_action(GameAction::Right),
        "the surviving pad keeps driving the same player"
    );
    assert_eq!(
        slot_of(&app, second),
        Some(1),
        "an unrelated device change must not reassign a bound pad"
    );
    assert_eq!(
        slot_of(&app, third),
        Some(0),
        "the newcomer takes the slot the departed pad freed"
    );
    assert_eq!(
        tick_actions(&mut app, 0),
        PlayerActions::from_action(GameAction::Left)
    );
}
