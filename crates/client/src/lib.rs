//! Public game-infrastructure surface shared by production and test apps.

#![forbid(unsafe_code)]

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

pub mod app_state;
pub mod bootstrap;
pub mod data;
pub mod i18n;
pub mod input;
pub mod settings;
pub mod simulation;

/// Project root plugin used by both the production client and startup smoke tests.
#[derive(Debug, Default)]
pub struct GameInfrastructurePlugin;

impl Plugin for GameInfrastructurePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<StatesPlugin>() {
            app.add_plugins(StatesPlugin);
        }
        app.add_plugins((
            app_state::AppStatePlugin,
            settings::SettingsPlugin,
            i18n::LocalizationPlugin,
            bootstrap::BootstrapPlugin,
            input::InputPlugin,
            simulation::SimulationPlugin,
        ));
    }
}
