//! Startup barrier for settings and localization.
//!
//! Both tasks reach `Resolved` whether they loaded cleanly or fell back to a
//! built-in default, so the barrier releases while diagnostics are kept.

use std::path::PathBuf;

use bevy::prelude::*;

use crate::app_state::{
    AppState, AppTransitionCause, AppTransitionRequests, arbitrate_transitions,
};
use crate::data::{DataCategory, DataErrorCause, DataLoadError, DataResolution, resolve_text};
use crate::i18n::{
    DEFAULT_LOCALE, Localization, SUPPORTED_LOCALES, builtin_english_catalog, parse_catalog,
};
use crate::settings::{SettingsError, SettingsLoad, SettingsStore, UserSettings};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BootstrapTaskState {
    #[default]
    Pending,
    Resolved,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Resource)]
pub struct BootstrapStatus {
    pub settings: BootstrapTaskState,
    pub localization: BootstrapTaskState,
}

impl BootstrapStatus {
    #[must_use]
    pub const fn is_ready(self) -> bool {
        matches!(self.settings, BootstrapTaskState::Resolved)
            && matches!(self.localization, BootstrapTaskState::Resolved)
    }
}

/// Where the startup loaders read from.
///
/// `settings: None` means the platform config directory; the asset root is
/// relative to the working directory the binary was launched from.
#[derive(Debug, Clone, Resource)]
pub struct BootstrapPaths {
    pub settings: Option<PathBuf>,
    pub asset_root: PathBuf,
}

impl Default for BootstrapPaths {
    fn default() -> Self {
        Self {
            settings: None,
            asset_root: PathBuf::from("assets"),
        }
    }
}

/// Diagnostics kept from a fallback so consumers can still see what failed.
#[derive(Debug, Default, Resource)]
pub struct BootstrapDiagnostics {
    pub settings: Option<SettingsError>,
    pub localization: Vec<DataLoadError>,
}

#[derive(Debug, Default)]
pub struct BootstrapPlugin;

impl Plugin for BootstrapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BootstrapStatus>()
            .init_resource::<BootstrapPaths>()
            .init_resource::<BootstrapDiagnostics>()
            .add_systems(Startup, (load_settings, load_localization).chain())
            .add_systems(
                Update,
                request_main_menu
                    .before(arbitrate_transitions)
                    .run_if(in_state(AppState::Boot)),
            );
    }
}

/// Resolve `UserSettings`, falling back to built-in defaults on any failure.
pub fn load_settings(
    paths: Res<BootstrapPaths>,
    mut commands: Commands,
    mut status: ResMut<BootstrapStatus>,
    mut diagnostics: ResMut<BootstrapDiagnostics>,
) {
    let store = match &paths.settings {
        Some(path) => Some(SettingsStore::new(path)),
        None => SettingsStore::platform_default(),
    };

    let load = store.map_or_else(
        || SettingsLoad::Defaulted {
            settings: UserSettings::default(),
            error: None,
        },
        |store| store.load(),
    );

    match load {
        SettingsLoad::Loaded(settings) => commands.insert_resource(settings),
        SettingsLoad::Defaulted { settings, error } => {
            diagnostics.settings = error;
            commands.insert_resource(settings);
        }
    }
    status.settings = BootstrapTaskState::Resolved;
}

/// Resolve the localization catalogs, keeping the English built-in as fallback.
pub fn load_localization(
    paths: Res<BootstrapPaths>,
    settings: Res<UserSettings>,
    mut commands: Commands,
    mut status: ResMut<BootstrapStatus>,
    mut diagnostics: ResMut<BootstrapDiagnostics>,
) {
    let mut catalogs = Vec::new();
    for locale in SUPPORTED_LOCALES {
        let path = paths.asset_root.join("i18n").join(format!("{locale}.json"));
        let resolution = resolve_text(
            path,
            DataCategory::Localization,
            builtin_english_catalog(),
            |source| parse_catalog(source).map_err(DataErrorCause::from),
        );
        match resolution {
            DataResolution::Loaded(catalog) => catalogs.push(catalog),
            DataResolution::Fallback { error, .. } => diagnostics.localization.push(error),
        }
    }

    // A usable English catalog is always available, even when every file failed.
    if !catalogs
        .iter()
        .any(|catalog| catalog.locale == DEFAULT_LOCALE)
    {
        catalogs.push(builtin_english_catalog());
    }

    let locale = if catalogs
        .iter()
        .any(|catalog| catalog.locale == settings.language)
    {
        settings.language.clone()
    } else {
        DEFAULT_LOCALE.to_string()
    };

    commands.insert_resource(Localization::new(locale, catalogs));
    status.localization = BootstrapTaskState::Resolved;
}

/// Release the startup barrier once, and only once both tasks are resolved.
pub fn request_main_menu(
    status: Res<BootstrapStatus>,
    mut requested: Local<bool>,
    mut requests: ResMut<AppTransitionRequests>,
) {
    if *requested || !status.is_ready() {
        return;
    }
    *requested = true;
    requests.submit(AppState::MainMenu, AppTransitionCause::BootstrapReady);
}
