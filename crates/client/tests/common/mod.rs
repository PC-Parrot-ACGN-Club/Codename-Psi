//! Shared harness for the client's Bevy integration tests.
//!
//! Fixed ticks are driven explicitly through `FixedUpdate` instead of wall-clock
//! accumulation: virtual time is paused so an ordinary `App::update` can never
//! advance the rules path on its own.

#![allow(dead_code)]

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use client::GameInfrastructurePlugin;
use client::app_state::{AppState, AppTransitionCause, AppTransitionRequests};
use client::input::{LocalInputSampler, PhysicalInput};
use client::settings::PlayerInputBindings;
use game_core::input::GameAction;

pub const ALL_STATES: [AppState; 8] = [
    AppState::Boot,
    AppState::MainMenu,
    AppState::ModeSelect,
    AppState::CharacterSelect,
    AppState::Settings,
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
    controlled_app_with_asset_root(WORKSPACE_ASSETS)
}

/// The real `assets/` tree.
///
/// Integration tests run with the package directory as their working
/// directory, so a plain "assets" would silently resolve to nothing and every
/// catalog would fall back instead of being read.
pub const WORKSPACE_ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

/// A controlled app whose asset reads come from `root`.
///
/// `AssetPlugin` is added before the project plugin, which then leaves it
/// alone -- the asset root is plugin configuration, not a per-load argument.
///
/// The settings path is pinned to a file that does not exist, so the app starts
/// from built-in defaults. Leaving it unset resolves the platform config
/// directory instead: every assertion about default bindings would then be an
/// assertion about whatever the developer running the suite last saved, and a
/// test that requested a save would overwrite their real settings file.
pub fn controlled_app_with_asset_root(root: impl Into<String>) -> App {
    assemble(root, false)
}

/// A controlled app with Bevy's UI stack installed, so pages and the HUD are
/// built as entities instead of being skipped.
///
/// The stack goes in before the project plugin on purpose: the client's UI, HUD
/// and effects plugins decide at build time whether there is a UI stack to draw
/// into, so one added afterwards would arrive too late to change their minds.
///
/// No renderer is involved. Layout and entity lifecycle run headlessly; only
/// putting pixels on a screen needs a GPU.
pub fn ui_app() -> App {
    let mut app = assemble(WORKSPACE_ASSETS, true);
    run_until_bootstrap_ready(&mut app);
    app
}

fn assemble(root: impl Into<String>, ui: bool) -> App {
    let mut app = App::new();
    app.insert_resource(client::bootstrap::BootstrapPaths {
        settings: Some(unused_settings_path()),
    });
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin {
            file_path: root.into(),
            ..default()
        },
    ));
    if ui {
        app.add_plugins((
            bevy::window::WindowPlugin::default(),
            bevy::picking::DefaultPickingPlugins,
            bevy::input_focus::InputFocusPlugin,
            bevy::input_focus::InputDispatchPlugin,
            bevy::text::TextPlugin,
            bevy::ui::UiPlugin,
        ));
        // Text layout reaches for the glyph atlas, which lives in image
        // assets; with no render plugin nothing else registers them.
        app.init_asset::<bevy::image::Image>();
        app.init_asset::<bevy::image::TextureAtlasLayout>();
    }
    app.add_plugins(GameInfrastructurePlugin);
    app.init_resource::<client::simulation::SimulationProbe>();
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app
}

/// A settings path inside a fresh temporary directory, with no file at it.
///
/// The directory is leaked rather than kept alive: a `TempDir` dropped at the
/// end of this function would delete itself before the app ever read the path,
/// and nothing is written there unless the test under way asks for a save.
fn unused_settings_path() -> std::path::PathBuf {
    tempfile::tempdir()
        .expect("a temporary directory for test settings")
        .keep()
        .join("settings.ron")
}

/// A client app driven by the production main schedule instead of by hand.
///
/// The harness above pauses time and runs `FixedUpdate` directly, which cannot
/// observe where sampling sits relative to the fixed loop -- it is the caller
/// that decides the order. Here time advances a fixed amount per `update()`, so
/// `RunFixedMainLoop` runs the ticks itself and the real ordering is under test.
pub fn production_schedule_app() -> App {
    let mut app = controlled_app();
    app.world_mut().resource_mut::<Time<Virtual>>().unpause();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(FRAME));
    app
}

/// One rules tick worth of wall clock, so one frame drives exactly one tick.
pub const FRAME: Duration = Duration::from_nanos(16_666_667);

/// Feed a key through the real event path.
///
/// Writing `ButtonInput` directly would not survive `keyboard_input_system`,
/// which clears the just-pressed set in `PreUpdate` before replaying events.
/// Only messages produce a press edge the capture system can observe, which is
/// exactly what a real device does.
pub fn send_key(app: &mut App, code: KeyCode, state: ButtonState) {
    let window = Entity::PLACEHOLDER;
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window,
    });
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
    if target == AppState::Settings {
        submit(app, AppState::Settings, AppTransitionCause::SettingsOpened);
        commit(app);
        assert_eq!(current_state(app), target);
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

/// Give `players` locals usable bindings.
///
/// The bindings go through `UserSettings`, because that is where the runtime
/// sampler mirrors from. They also have to be written *after* the startup
/// barrier: the bootstrap load replaces the whole settings resource, which
/// would otherwise discard them again.
pub fn install_sampler(app: &mut App, players: usize) {
    run_until_bootstrap_ready(app);
    {
        let mut settings = app
            .world_mut()
            .resource_mut::<client::settings::UserSettings>();
        for player in 0..players {
            settings.players[player] = keyboard_bindings(player);
        }
    }
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
