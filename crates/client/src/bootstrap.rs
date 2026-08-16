//! Startup barrier for settings and localization.
//!
//! Both tasks reach `Resolved` whether they loaded cleanly or failed. Settings
//! own their first-run default; localization resolves through its key fallback.

use std::path::PathBuf;
use std::time::Duration;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::app_state::{AppState, AppTransitionCause, AppTransitionRequests, AppTransitionSet};
use crate::data::{
    DataCategory, DataErrorCause, DataLoadError, DataResolution, SourceText, resolve_source,
};
use crate::i18n::{DEFAULT_LOCALE, Localization, SUPPORTED_LOCALES, parse_catalog};
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
/// Only settings live here: they are stored in the platform config directory
/// rather than under `assets/`, so they are outside the asset pipeline. The
/// asset root belongs to Bevy's `AssetPlugin`. `None` means the platform
/// default config directory.
#[derive(Debug, Clone, Default, Resource)]
pub struct BootstrapPaths {
    pub settings: Option<PathBuf>,
}

/// How long the barrier waits for a startup asset before resolving failure.
///
/// Asset reads are asynchronous, so without a deadline a source that never
/// resolves would strand the app in `Boot`. Timing out is treated as a load
/// failure: the localization key fallback remains available and the barrier
/// releases.
pub const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(5);

/// The in-flight localization reads, plus how long they have been waiting.
#[derive(Debug, Resource)]
pub struct LocalizationLoad {
    handles: Vec<(String, Handle<SourceText>)>,
    waited: Duration,
}

/// What a bootstrap step writes: the resolved value, its state, its diagnostics.
///
/// Grouped so a step takes one output parameter instead of three.
#[derive(bevy::ecs::system::SystemParam)]
pub struct BootstrapOutput<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub status: ResMut<'w, BootstrapStatus>,
    pub diagnostics: ResMut<'w, BootstrapDiagnostics>,
}

/// Diagnostics kept from a failed startup read.
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
            .add_systems(Startup, (load_settings, start_localization_load).chain())
            .add_systems(
                Update,
                (poll_localization, request_main_menu)
                    .chain()
                    .in_set(AppTransitionSet::Request)
                    .run_if(in_state(AppState::Boot)),
            );
    }
}

/// Resolve `UserSettings`, falling back to built-in defaults on any failure.
///
/// This is the one door from the persisted document into memory, so the binding
/// invariants are repaired here: a file written before they were enforced parses
/// fine and is still wrong to play on.
pub fn load_settings(
    paths: Res<BootstrapPaths>,
    mut commands: Commands,
    mut status: ResMut<BootstrapStatus>,
    mut diagnostics: ResMut<BootstrapDiagnostics>,
    mut save: MessageWriter<crate::settings::SaveSettingsRequest>,
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

    let mut settings = match load {
        SettingsLoad::Loaded(settings) => settings,
        SettingsLoad::Defaulted { settings, error } => {
            diagnostics.settings = error;
            settings
        }
    };
    let dropped = settings.normalize_bindings();
    if !dropped.is_empty() {
        for binding in &dropped {
            warn!(
                "dropped P{} {:?} binding {}: another binding already holds it",
                binding.player + 1,
                binding.action,
                binding.input.name()
            );
        }
        // Written back so the repair is durable. Leaving it in memory alone
        // would repair every launch and fix nothing, and the file would keep
        // its extra bindings for as long as the player never edits a setting.
        save.write(crate::settings::SaveSettingsRequest);
    }
    commands.insert_resource(settings);
    status.settings = BootstrapTaskState::Resolved;
}

/// Ask Bevy Asset for every supported catalog.
pub fn start_localization_load(asset_server: Res<AssetServer>, mut commands: Commands) {
    let handles = SUPPORTED_LOCALES
        .iter()
        .map(|locale| {
            (
                (*locale).to_string(),
                asset_server.load::<SourceText>(format!("i18n/{locale}.json")),
            )
        })
        .collect();
    commands.insert_resource(LocalizationLoad {
        handles,
        waited: Duration::ZERO,
    });
}

/// Resolve the localization catalogs once their reads settle, or on timeout.
///
/// Reading is asynchronous, so this polls until every catalog has either been
/// read or failed. Parsing stays here rather than in the asset loader: that is
/// what keeps `Parse`, `UnsupportedSchema` and `InvalidData` distinguishable
/// from Bevy's opaque read failure.
pub fn poll_localization(
    time: Res<Time<Real>>,
    asset_server: Res<AssetServer>,
    sources: Res<Assets<SourceText>>,
    settings: Res<UserSettings>,
    mut load: ResMut<LocalizationLoad>,
    mut out: BootstrapOutput,
) {
    let BootstrapOutput {
        commands,
        status,
        diagnostics,
    } = &mut out;

    if status.localization == BootstrapTaskState::Resolved {
        return;
    }

    load.waited += time.delta();
    let timed_out = load.waited >= BOOTSTRAP_TIMEOUT;

    let mut catalogs = Vec::new();
    let mut errors = Vec::new();
    for (locale, handle) in &load.handles {
        let path = format!("i18n/{locale}.json");
        let source = match asset_server.load_state(handle) {
            LoadState::Loaded => sources
                .get(handle)
                .map(|text| Ok(text.0.as_str()))
                .unwrap_or(Err(DataErrorCause::Io("asset dropped after load".into()))),
            LoadState::Failed(error) => Err(DataErrorCause::Io(error.to_string())),
            _ if timed_out => Err(DataErrorCause::Io(format!(
                "timed out after {}s",
                BOOTSTRAP_TIMEOUT.as_secs()
            ))),
            // Still reading: nothing to resolve this frame.
            _ => return,
        };

        match resolve_source(&path, DataCategory::Localization, source, |text| {
            parse_catalog(text).map_err(DataErrorCause::from)
        }) {
            DataResolution::Loaded(catalog) => catalogs.push(catalog),
            DataResolution::Failed(error) => errors.push(error),
        }
    }

    diagnostics.localization.extend(errors);

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
