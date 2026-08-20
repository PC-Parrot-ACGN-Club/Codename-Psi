//! Bevy application entry point
//!
//! Owns windowing, rendering, and asset paths. Rules logic stays in `game_core`;
//! networking stays behind the optional `net` feature.

use bevy::prelude::*;
use client::GameInfrastructurePlugin;

/// Log targets silenced on top of Bevy's own defaults.
///
/// Laying out any CJK text asks ICU for the Chinese/Japanese line-breaking
/// dictionary, which the shipped ICU data does not carry. The lookup fails and
/// warns once per text run, so a `zh-CN` start prints a wall of identical
/// warnings. Line breaking falls back to breaking between characters, which is
/// what CJK does anyway, so the warning reports nothing the player or the
/// developer can act on.
const EXTRA_LOG_FILTER: &str = "icu_provider=off";

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Codename Psi".into(),
                    ..default()
                }),
                ..default()
            })
            .set(bevy::log::LogPlugin {
                filter: format!("{},{EXTRA_LOG_FILTER}", bevy::log::DEFAULT_FILTER),
                ..default()
            }),
    )
    .add_plugins(GameInfrastructurePlugin)
    .add_systems(Startup, setup);

    #[cfg(feature = "ci_testing")]
    app.add_systems(
        OnEnter(client::app_state::AppState::MainMenu),
        reached_main_menu,
    );

    app.run();
}

/// Marker for the bounded startup run, printed once the app is past `Boot`.
///
/// The exit event alone only proves the process came down cleanly; a build that
/// never left `Boot` would exit just as successfully. The release workflow
/// requires this line as well, which is what makes the run an acceptance of
/// reaching `MainMenu` rather than of merely starting.
#[cfg(feature = "ci_testing")]
fn reached_main_menu() {
    info!("startup-smoke: reached MainMenu");
}

fn setup(mut commands: Commands) {
    // Minimal smoke startup: a 2D camera proves the Bevy app links and boots.
    commands.spawn(Camera2d);
}
