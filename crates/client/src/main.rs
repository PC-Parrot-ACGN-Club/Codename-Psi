//! Bevy application entry point
//!
//! Owns windowing, rendering, and asset paths. Rules logic stays in `core`;
//! networking stays behind the optional `net` feature.

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Codename Psi".into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // Minimal smoke startup: a 2D camera proves the Bevy app links and boots.
    commands.spawn(Camera2d);
}
