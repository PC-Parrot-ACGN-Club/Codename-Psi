//! Shared harness for the client's Bevy integration tests.
//!
//! Fixed ticks are driven explicitly through `FixedUpdate` instead of wall-clock
//! accumulation: virtual time is paused so an ordinary `App::update` can never
//! advance the rules path on its own.

#![allow(dead_code)]

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use client::GameInfrastructurePlugin;
use client::app_state::{AppState, AppTransitionCause, AppTransitionRequests};
use client::input::{LocalInputSampler, PhysicalInput};
use client::settings::PlayerInputBindings;
use game_core::input::GameAction;

pub const ALL_STATES: [AppState; 7] = [
    AppState::Boot,
    AppState::MainMenu,
    AppState::ModeSelect,
    AppState::CharacterSelect,
    AppState::Match,
    AppState::Paused,
    AppState::Result,
];

/// A minimal app with only the state machine, for pure transition observation.
pub fn state_only_app() -> App {
    let mut app = App::new();
    app.add_plugins((StatesPlugin, client::app_state::AppStatePlugin));
    app
}

/// A minimal client app running the real project root plugin under controlled time.
///
/// The simulation probe is opt-in instrumentation the production plugin does
/// not register, so tests that observe the fixed schedule insert it here.
pub fn controlled_app() -> App {
    controlled_app_with_asset_root("assets")
}

/// A controlled app whose asset reads come from `root`.
///
/// `AssetPlugin` is added before the project plugin, which then leaves it
/// alone -- the asset root is plugin configuration, not a per-load argument.
pub fn controlled_app_with_asset_root(root: impl Into<String>) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin {
            file_path: root.into(),
            ..default()
        },
        GameInfrastructurePlugin,
    ));
    app.init_resource::<client::simulation::SimulationProbe>();
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app
}

/// Pump frames until the startup barrier resolves.
///
/// Asset reads are asynchronous, so a single `update()` is not enough. The cap
/// only stops a broken build from hanging the suite; the production timeout is
/// what guarantees the barrier releases.
pub fn run_until_bootstrap_ready(app: &mut App) {
    for _ in 0..2000 {
        app.update();
        if app
            .world()
            .resource::<client::bootstrap::BootstrapStatus>()
            .is_ready()
        {
            return;
        }
    }
    panic!("the startup barrier never resolved");
}

pub fn current_state(app: &App) -> AppState {
    *app.world().resource::<State<AppState>>().get()
}

pub fn submit(app: &mut App, target: AppState, cause: AppTransitionCause) {
    app.world_mut()
        .resource_mut::<AppTransitionRequests>()
        .submit(target, cause);
}

/// Run a full state-commit cycle: arbitration happens in `Update`, and the
/// queued `NextState` is applied by the following frame's state transition.
pub fn commit(app: &mut App) {
    app.update();
    app.update();
}

/// Give the fixed rules path exactly one controlled execution opportunity.
pub fn run_fixed_tick(app: &mut App) {
    app.world_mut().run_schedule(FixedUpdate);
}

pub fn run_fixed_ticks(app: &mut App, ticks: usize) {
    for _ in 0..ticks {
        run_fixed_tick(app);
    }
}

/// Walk the app from `Boot` to `target` along valid edges only.
pub fn advance_to(app: &mut App, target: AppState) {
    if target == AppState::Boot {
        return;
    }

    // Requested explicitly so the walk works with or without the bootstrap
    // barrier installed; an identical bootstrap request de-duplicates.
    submit(app, AppState::MainMenu, AppTransitionCause::BootstrapReady);
    commit(app);
    assert_eq!(current_state(app), AppState::MainMenu);
    if target == AppState::MainMenu {
        return;
    }

    submit(app, AppState::ModeSelect, AppTransitionCause::StartGame);
    commit(app);
    if target == AppState::ModeSelect {
        return;
    }

    submit(
        app,
        AppState::CharacterSelect,
        AppTransitionCause::ModeConfirmed,
    );
    commit(app);
    if target == AppState::CharacterSelect {
        return;
    }

    submit(app, AppState::Match, AppTransitionCause::CharacterConfirmed);
    commit(app);
    match target {
        AppState::Match => {}
        AppState::Paused => {
            submit(app, AppState::Paused, AppTransitionCause::PauseRequested);
            commit(app);
        }
        AppState::Result => {
            submit(app, AppState::Result, AppTransitionCause::MatchCompleted);
            commit(app);
        }
        other => panic!("no valid walk to {other:?}"),
    }
    assert_eq!(current_state(app), target);
}

/// Keyboard bindings for the four configurable actions of one local player.
pub fn keyboard_bindings(player: usize) -> PlayerInputBindings {
    let mut bindings = PlayerInputBindings::default();
    for (action, code) in [
        (GameAction::SoftDrop, "SoftDrop"),
        (GameAction::HardDrop, "HardDrop"),
        (GameAction::RotateClockwise, "RotateCw"),
        (GameAction::RotateCounterClockwise, "RotateCcw"),
    ] {
        bindings.bindings.insert(
            action,
            vec![PhysicalInput::keyboard(format!("P{player}{code}"))],
        );
    }
    bindings
}

pub fn binding_input(player: usize, action: GameAction) -> PhysicalInput {
    let code = match action {
        GameAction::SoftDrop => "SoftDrop",
        GameAction::HardDrop => "HardDrop",
        GameAction::RotateClockwise => "RotateCw",
        GameAction::RotateCounterClockwise => "RotateCcw",
        other => panic!("{other:?} is a fixed binding, not a configurable one"),
    };
    PhysicalInput::keyboard(format!("P{player}{code}"))
}

/// Replace the sampler with one that has usable bindings for `players` locals.
pub fn install_sampler(app: &mut App, players: usize) {
    app.insert_resource(LocalInputSampler::new(
        (0..players).map(keyboard_bindings).collect(),
    ));
}

pub fn press(app: &mut App, player: usize, action: GameAction) {
    let input = binding_input(player, action);
    app.world_mut()
        .resource_mut::<LocalInputSampler>()
        .press(player, input);
}

pub fn release(app: &mut App, player: usize, action: GameAction) {
    let input = binding_input(player, action);
    app.world_mut()
        .resource_mut::<LocalInputSampler>()
        .release(player, &input);
}
