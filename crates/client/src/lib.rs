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
        // Device sampling needs the keyboard and gamepad resources. Supplying
        // them here keeps the headless smoke on the same assembly as the
        // production client, where `DefaultPlugins` already provides them.
        if !app.is_plugin_added::<bevy::input::InputPlugin>() {
            app.add_plugins(bevy::input::InputPlugin);
        }
        // Runtime data is read through Bevy Asset. A test that needs a
        // different asset root adds `AssetPlugin` itself before this plugin.
        if !app.is_plugin_added::<bevy::asset::AssetPlugin>() {
            app.add_plugins(bevy::asset::AssetPlugin::default());
        }
        app.add_plugins((
            data::DataPlugin,
            app_state::AppStatePlugin,
            settings::SettingsPlugin,
            i18n::LocalizationPlugin,
            bootstrap::BootstrapPlugin,
            input::InputPlugin,
            simulation::SimulationPlugin,
        ));
    }
}
