//! Where device sampling sits relative to the fixed loop, under the real
//! main schedule.
//!
//! The other input tests drive `FixedUpdate` by hand, so they fix the ordering
//! themselves and cannot catch sampling running on the wrong side of the fixed
//! loop. These run the production schedule and let Bevy order the two.

mod common;
mod presentation_common;

use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use client::app_state::AppState;
use client::simulation::CurrentTickInputs;
use game_core::input::{GameAction, PlayerActions};

use common::{FRAME, advance_to, production_schedule_app, send_key};

/// P1 keyboard defaults: `A`/`D` move, `W` hard drops.
const LEFT: KeyCode = KeyCode::KeyA;
const HARD_DROP: KeyCode = KeyCode::KeyW;

/// P2 keyboard defaults: arrows move, `Numpad1` rotates counter-clockwise.
const P2_LEFT: KeyCode = KeyCode::ArrowLeft;
const P2_ROTATE_CCW: KeyCode = KeyCode::Numpad1;

/// The canonical actions the last fixed tick consumed.
fn last_tick(app: &App) -> PlayerActions {
    app.world()
        .resource::<CurrentTickInputs>()
        .inputs
        .player(0)
        .expect("player 0 must exist")
}

fn ticks_run(app: &App) -> usize {
    app.world()
        .resource::<client::simulation::SimulationProbe>()
        .consumed_inputs
        .len()
}

// integration-system/input-and-fixed-tick::TC-011
#[test]
fn input_from_a_frame_reaches_that_frames_fixed_tick() {
    let mut app = production_schedule_app();
    advance_to(&mut app, AppState::Match);

    send_key(&mut app, LEFT, ButtonState::Pressed);
    app.update();
    assert_eq!(
        last_tick(&app),
        PlayerActions::from_action(GameAction::Left),
        "the tick in the press frame must already see the action, not the next frame's"
    );

    send_key(&mut app, LEFT, ButtonState::Released);
    app.update();
    assert_eq!(
        last_tick(&app),
        PlayerActions::EMPTY,
        "the tick in the release frame must no longer see the action"
    );
}

// integration-system/input-and-fixed-tick::TC-012
#[test]
fn several_fixed_ticks_in_one_frame_share_that_frames_sample() {
    let mut app = production_schedule_app();
    advance_to(&mut app, AppState::Match);

    // Hold a continuous action and produce one un-submitted press edge, then
    // let a single frame owe three ticks.
    send_key(&mut app, LEFT, ButtonState::Pressed);
    send_key(&mut app, HARD_DROP, ButtonState::Pressed);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(3 * FRAME));

    let before = ticks_run(&app);
    app.update();

    let probe = app
        .world()
        .resource::<client::simulation::SimulationProbe>();
    let frame_ticks: Vec<PlayerActions> = probe.consumed_inputs[before..]
        .iter()
        .map(|inputs| inputs.player(0).expect("player 0 must exist"))
        .collect();

    assert_eq!(frame_ticks.len(), 3, "one frame owed three fixed ticks");
    assert!(
        frame_ticks
            .iter()
            .all(|actions| actions.contains(GameAction::Left)),
        "a held continuous action holds across every tick of the frame: {frame_ticks:?}"
    );
    assert!(
        frame_ticks[0].contains(GameAction::HardDrop),
        "the press edge commits in the first tick of the frame"
    );
    assert!(
        frame_ticks[1..]
            .iter()
            .all(|actions| !actions.contains(GameAction::HardDrop)),
        "one press edge must not repeat across the frame's remaining ticks: {frame_ticks:?}"
    );
}

// integration-system/input-and-fixed-tick::TC-013
#[test]
fn a_press_that_ends_in_the_same_frame_still_reaches_the_rules_layer() {
    // Every one-shot action, through its P1 default binding.
    for (code, action) in [
        (HARD_DROP, GameAction::HardDrop),
        (KeyCode::KeyK, GameAction::RotateClockwise),
        (KeyCode::KeyJ, GameAction::RotateCounterClockwise),
    ] {
        let mut app = production_schedule_app();
        advance_to(&mut app, AppState::Match);

        // Pressed and released before the capture system ever runs: nothing is
        // held by then, so only the edge can carry this input.
        send_key(&mut app, code, ButtonState::Pressed);
        send_key(&mut app, code, ButtonState::Released);
        app.update();

        assert!(
            last_tick(&app).contains(action),
            "{action:?} was pressed and released within one frame and must still produce one action"
        );

        app.update();
        assert!(
            !last_tick(&app).contains(action),
            "{action:?} must not repeat after its single press edge was consumed"
        );
    }
}

/// A local-versus app already running a real rules instance, under the
/// production schedule.
///
/// The mode matters: under `SinglePlayer` slot 1 is overwritten by the AI, so
/// nothing the second player pressed would be visible in the tick's inputs.
fn local_versus_app(seed: u64) -> App {
    let mut app = production_schedule_app();
    app.insert_resource(client::match_flow::SelectedMode(
        client::page::MatchMode::LocalVersus,
    ));
    app.insert_resource(client::match_flow::FrozenMatch(presentation_common::spec(
        seed,
    )));
    advance_to(&mut app, AppState::Match);

    // The round opens with a countdown, during which no action is consumed.
    for _ in 0..600 {
        if app
            .world()
            .resource::<client::simulation::RulesSimulation>()
            .0
            .phase()
            .is_playing()
        {
            return app;
        }
        app.update();
    }
    panic!("the round never opened for play");
}

/// The column each slot's active group currently pivots on.
fn pivot_columns(app: &App) -> [u8; 2] {
    let simulation = app
        .world()
        .resource::<client::simulation::RulesSimulation>();
    [0, 1].map(|slot| {
        simulation
            .0
            .active_group(slot)
            .expect("both slots control a group while playing")
            .pivot()
            .x()
    })
}

// integration-system/input-and-fixed-tick::TC-016
#[test]
fn player_twos_keys_drive_slot_one_and_leave_slot_zero_alone() {
    let mut app = local_versus_app(31);
    let before = pivot_columns(&app);

    send_key(&mut app, P2_LEFT, ButtonState::Pressed);
    send_key(&mut app, P2_ROTATE_CCW, ButtonState::Pressed);
    app.update();

    let tick = app.world().resource::<CurrentTickInputs>().inputs;
    let p2 = tick.player(1).expect("player 1 must exist");
    assert!(
        p2.contains(GameAction::Left) && p2.contains(GameAction::RotateCounterClockwise),
        "P2's fixed direction and P2's own configurable binding both reach slot 1: {p2:?}"
    );
    assert_eq!(
        tick.player(0),
        Some(PlayerActions::EMPTY),
        "no key P2 pressed may reach slot 0"
    );

    let after = pivot_columns(&app);
    assert_eq!(
        after[1],
        before[1] - 1,
        "slot 1's group moved one column left"
    );
    assert_eq!(after[0], before[0], "slot 0's group did not move");
}

// integration-system/input-and-fixed-tick::TC-016
#[test]
fn player_ones_keys_drive_slot_zero_and_leave_slot_one_alone() {
    let mut app = local_versus_app(31);
    let before = pivot_columns(&app);

    send_key(&mut app, LEFT, ButtonState::Pressed);
    app.update();

    let tick = app.world().resource::<CurrentTickInputs>().inputs;
    assert!(
        tick.player(0)
            .expect("player 0 must exist")
            .contains(GameAction::Left)
    );
    assert_eq!(tick.player(1), Some(PlayerActions::EMPTY));

    let after = pivot_columns(&app);
    assert_eq!(
        after[0],
        before[0] - 1,
        "slot 0's group moved one column left"
    );
    assert_eq!(after[1], before[1], "slot 1's group did not move");
}
