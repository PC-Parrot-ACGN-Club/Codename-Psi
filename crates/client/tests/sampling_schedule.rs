//! Where device sampling sits relative to the fixed loop, under the real
//! main schedule.
//!
//! The other input tests drive `FixedUpdate` by hand, so they fix the ordering
//! themselves and cannot catch sampling running on the wrong side of the fixed
//! loop. These run the production schedule and let Bevy order the two.

mod common;

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

// docs/test/game-infrastructure.md TC-067
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

// docs/test/game-infrastructure.md TC-068
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

// docs/test/game-infrastructure.md TC-069
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
