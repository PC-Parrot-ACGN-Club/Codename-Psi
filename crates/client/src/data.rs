//! Runtime data resolution and error-context types.
//!
//! Reading and parsing are deliberately separate. Bevy Asset owns reading, so
//! the client gets one asset pipeline and one asset root; parsing stays in
//! `game_core::config` and the client parsers, which is what keeps the four
//! typed causes (`Io`, `Parse`, `UnsupportedSchema`, `InvalidData`)
//! distinguishable. An `AssetLoader` that parsed as well would collapse the
//! last three into Bevy's opaque load failure.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::asset::io::Reader;
use bevy::asset::{Asset, AssetLoader, LoadContext, LoadState};
use bevy::prelude::*;
use game_core::config::{ConfigError, RulesStub, STUB_SCHEMA_VERSION, parse_rules_stub};

use crate::i18n::CatalogError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Which `assets/` data class a load belongs to.
///
/// User settings are not listed: they live in the platform config directory,
/// not under `assets/`, and report failures as `SettingsError`.
pub enum DataCategory {
    Rules,
    Localization,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataErrorCause {
    Io(String),
    Parse(String),
    UnsupportedSchema { found: u32, supported: u32 },
    InvalidData(String),
}

/// Classify a `game_core` parser failure without losing its reason.
impl From<ConfigError> for DataErrorCause {
    fn from(error: ConfigError) -> Self {
        match error {
            ConfigError::Ron(reason) | ConfigError::Json(reason) => Self::Parse(reason),
            ConfigError::UnsupportedSchema { found, supported } => {
                Self::UnsupportedSchema { found, supported }
            }
            ConfigError::InvalidData(reason) => Self::InvalidData(reason),
        }
    }
}

/// Classify a client localization parser failure without losing its reason.
impl From<CatalogError> for DataErrorCause {
    fn from(error: CatalogError) -> Self {
        match error {
            CatalogError::Parse(reason) => Self::Parse(reason),
            CatalogError::UnsupportedSchema { found, supported } => {
                Self::UnsupportedSchema { found, supported }
            }
            invalid @ CatalogError::InvalidData { .. } => Self::InvalidData(invalid.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLoadError {
    pub path: PathBuf,
    pub category: DataCategory,
    pub cause: DataErrorCause,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataResolution<T> {
    Loaded(T),
    Fallback { value: T, error: DataLoadError },
}

impl<T> DataResolution<T> {
    /// Both variants are resolved, so consumers always get a value; `error`
    /// distinguishes a clean load from a fallback.
    #[must_use]
    pub fn value(&self) -> &T {
        match self {
            Self::Loaded(value) | Self::Fallback { value, .. } => value,
        }
    }

    #[must_use]
    pub fn error(&self) -> Option<&DataLoadError> {
        match self {
            Self::Loaded(_) => None,
            Self::Fallback { error, .. } => Some(error),
        }
    }
}

/// Raw source text read from `assets/` by Bevy Asset.
///
/// The loader performs no parsing: it is the "source text/bytes" hand-off in
/// the loading contract, and every schema or semantic judgement happens in the
/// parser that consumes it.
#[derive(Debug, Clone, PartialEq, Eq, Asset, TypePath)]
pub struct SourceText(pub String);

/// Reads any project data file into a [`SourceText`].
#[derive(Debug, Default, TypePath)]
pub struct SourceTextLoader;

/// Failure to read a source file through Bevy Asset.
#[derive(Debug, thiserror::Error)]
pub enum SourceTextError {
    #[error("failed to read asset: {0}")]
    Io(#[from] std::io::Error),
    #[error("asset is not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

impl AssetLoader for SourceTextLoader {
    type Asset = SourceText;
    type Settings = ();
    type Error = SourceTextError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(SourceText(String::from_utf8(bytes)?))
    }
}

/// Where the rules document lives under `assets/`.
///
/// The path belongs here rather than to any consumer: `client::data` owns the
/// asset root and the load lifecycle, and consumers only read the resolution.
pub const RULES_PATH: &str = "data/rules.stub.ron";

/// How long a data read may hang before it is treated as failed.
///
/// Without this a stalled read would leave consumers with no typed data at
/// all, which is the one outcome the contract rules out. Matches the startup
/// barrier's timeout; unlike the barrier, this load does not gate `Boot`.
pub const DATA_LOAD_TIMEOUT: Duration = Duration::from_secs(5);

/// Rules used when the file cannot be read or parsed.
fn builtin_rules() -> RulesStub {
    RulesStub {
        schema_version: STUB_SCHEMA_VERSION,
        id: "builtin".into(),
    }
}

/// A rules read in flight; removed once the resolution is published.
#[derive(Debug, Resource)]
pub struct RulesLoad {
    handle: Handle<SourceText>,
    waited: Duration,
}

/// The resolved rules document, in the form consumers read.
///
/// Present only once the load settles. Both resolutions carry a usable value,
/// so a consumer never has to handle "loaded but empty".
#[derive(Debug, Resource)]
pub struct RulesData(pub DataResolution<RulesStub>);

impl RulesData {
    #[must_use]
    pub fn rules(&self) -> &RulesStub {
        self.0.value()
    }

    /// The read or parse failure behind a fallback, if this is one.
    #[must_use]
    pub fn error(&self) -> Option<&DataLoadError> {
        self.0.error()
    }
}

/// Ask Bevy Asset for the rules document.
pub fn start_rules_load(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.insert_resource(RulesLoad {
        handle: asset_server.load::<SourceText>(RULES_PATH),
        waited: Duration::ZERO,
    });
}

/// Publish the rules resolution once the read settles, or on timeout.
///
/// Parsing stays here rather than in the asset loader, for the reason given at
/// the top of this module: it is what keeps the typed causes apart.
pub fn poll_rules(
    time: Res<Time<Real>>,
    asset_server: Res<AssetServer>,
    sources: Res<Assets<SourceText>>,
    mut load: ResMut<RulesLoad>,
    mut commands: Commands,
) {
    load.waited += time.delta();
    let timed_out = load.waited >= DATA_LOAD_TIMEOUT;

    let source = match asset_server.load_state(&load.handle) {
        LoadState::Loaded => sources
            .get(&load.handle)
            .map(|text| Ok(text.0.as_str()))
            .unwrap_or(Err(DataErrorCause::Io("asset dropped after load".into()))),
        LoadState::Failed(error) => Err(DataErrorCause::Io(error.to_string())),
        _ if timed_out => Err(DataErrorCause::Io(format!(
            "timed out after {}s",
            DATA_LOAD_TIMEOUT.as_secs()
        ))),
        // Still reading: nothing to resolve this frame.
        _ => return,
    };

    commands.insert_resource(RulesData(resolve_source(
        RULES_PATH,
        DataCategory::Rules,
        builtin_rules(),
        source,
        |text| parse_rules_stub(text).map_err(DataErrorCause::from),
    )));
    commands.remove_resource::<RulesLoad>();
}

/// Registers the project's asset reading path and data load lifecycle.
#[derive(Debug, Default)]
pub struct DataPlugin;

impl Plugin for DataPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SourceText>()
            .register_asset_loader(SourceTextLoader)
            .add_systems(Startup, start_rules_load)
            .add_systems(Update, poll_rules.run_if(resource_exists::<RulesLoad>));
    }
}

/// Turn a read source into a resolution, keeping the failure cause typed.
///
/// Separated from the asset polling so the mapping from parser error to
/// `DataLoadError` is testable without an asset server.
pub fn resolve_source<T>(
    path: impl AsRef<Path>,
    category: DataCategory,
    fallback: T,
    source: Result<&str, DataErrorCause>,
    parser: impl FnOnce(&str) -> Result<T, DataErrorCause>,
) -> DataResolution<T> {
    let path = path.as_ref();
    let outcome = source.and_then(parser);
    match outcome {
        Ok(value) => DataResolution::Loaded(value),
        Err(cause) => DataResolution::Fallback {
            value: fallback,
            error: DataLoadError {
                path: path.into(),
                category,
                cause,
            },
        },
    }
}
