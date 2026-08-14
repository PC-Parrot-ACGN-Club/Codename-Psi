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
use game_core::config::{
    ConfigError, ValidatedRuleLibrary, parse_character_play, parse_fever_puzzle_book, parse_roster,
    parse_rule_profile,
};

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
            // A violated semantic constraint is invalid data with a located
            // cause; the field path and layer stay in the message.
            validation @ ConfigError::Validation { .. } => {
                Self::InvalidData(validation.to_string())
            }
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
    Failed(DataLoadError),
}

impl<T> DataResolution<T> {
    /// Returns loaded data when this resolution succeeded.
    #[must_use]
    pub const fn loaded(&self) -> Option<&T> {
        match self {
            Self::Loaded(value) => Some(value),
            Self::Failed(_) => None,
        }
    }

    #[must_use]
    pub fn error(&self) -> Option<&DataLoadError> {
        match self {
            Self::Loaded(_) => None,
            Self::Failed(error) => Some(error),
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

/// Where the rule profile lives under `assets/`.
///
/// The paths belong here rather than to any consumer: `client::data` owns the
/// asset root and the load lifecycle, and consumers only read the resolution.
pub const RULES_PROFILE_PATH: &str = "data/rules/profiles/fever.ron";
/// Where the character roster lives under `assets/`.
pub const RULES_ROSTER_PATH: &str = "data/rules/roster.ron";
/// Where the Fever puzzle book lives under `assets/`.
pub const RULES_PUZZLE_BOOK_PATH: &str = "data/rules/puzzles/fever-r1.ron";
/// Where each character's gameplay data lives under `assets/`.
pub const RULES_PLAY_PATHS: [&str; 2] = [
    "data/rules/play/fever-r1/psi-a.ron",
    "data/rules/play/fever-r1/psi-b.ron",
];

/// Every rules path, profile first, then roster, puzzle book and plays.
#[must_use]
pub fn rules_paths() -> Vec<&'static str> {
    let mut paths = vec![
        RULES_PROFILE_PATH,
        RULES_ROSTER_PATH,
        RULES_PUZZLE_BOOK_PATH,
    ];
    paths.extend(RULES_PLAY_PATHS);
    paths
}

/// How long a data read may hang before it is treated as failed.
pub const DATA_LOAD_TIMEOUT: Duration = Duration::from_secs(5);

/// A rules read in flight; removed once the resolution is published.
#[derive(Debug, Resource)]
pub struct RulesLoad {
    handles: Vec<Handle<SourceText>>,
    waited: Duration,
}

/// The resolved rules document, in the form consumers read.
///
/// Present only once the load settles. A failed rules resolution deliberately
/// has no substitute: the client must not start a match without authority.
#[derive(Debug, Resource)]
pub struct RulesData {
    /// The library, or the failure that blocks every match.
    pub resolution: DataResolution<ValidatedRuleLibrary>,
    /// Per-character failures that only narrow character selection.
    ///
    /// The blocking scope follows the same criterion as the level itself: a
    /// missing profile makes Match unreachable, while one character's
    /// unusable gameplay data only makes that character unselectable.
    pub excluded_characters: Vec<DataLoadError>,
}

impl RulesData {
    /// The validated library, when the blocking-scope data all loaded.
    #[must_use]
    pub fn rules(&self) -> Option<&ValidatedRuleLibrary> {
        self.resolution.loaded()
    }

    /// The read or parse failure, if this is one.
    #[must_use]
    pub fn error(&self) -> Option<&DataLoadError> {
        self.resolution.error()
    }

    /// Whether a match may be started from this resolution.
    #[must_use]
    pub const fn is_playable(&self) -> bool {
        matches!(self.resolution, DataResolution::Loaded(_))
    }
}

/// Ask Bevy Asset for the rules documents.
pub fn start_rules_load(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.insert_resource(RulesLoad {
        handles: rules_paths()
            .into_iter()
            .map(|path| asset_server.load::<SourceText>(path))
            .collect(),
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

    let source_for = |index: usize| -> Option<Result<&str, DataErrorCause>> {
        Some(match asset_server.load_state(&load.handles[index]) {
            LoadState::Loaded => sources
                .get(&load.handles[index])
                .map(|text| Ok(text.0.as_str()))
                .unwrap_or(Err(DataErrorCause::Io("asset dropped after load".into()))),
            LoadState::Failed(error) => Err(DataErrorCause::Io(error.to_string())),
            _ if timed_out => Err(DataErrorCause::Io(format!(
                "timed out after {}s",
                DATA_LOAD_TIMEOUT.as_secs()
            ))),
            _ => return None,
        })
    };
    let paths = rules_paths();
    let mut source_texts = Vec::with_capacity(paths.len());
    for index in 0..paths.len() {
        let Some(source) = source_for(index) else {
            return;
        };
        source_texts.push(source);
    }

    // Blocking scope: the profile, roster and puzzle book gate every match, so
    // a failure in any of them fails the whole resolution. A character's
    // gameplay data only gates that character.
    let mut excluded_characters = Vec::new();
    let resolution = match build_library(&source_texts, &paths, &mut excluded_characters) {
        Ok(library) => DataResolution::Loaded(library),
        Err(error) => DataResolution::Failed(error),
    };

    for excluded in &excluded_characters {
        warn!("excluded unusable character data at {:?}", excluded.path);
    }
    commands.insert_resource(RulesData {
        resolution,
        excluded_characters,
    });
    commands.remove_resource::<RulesLoad>();
}

/// Parses one blocking-scope document, attaching its resource context.
fn parse_blocking<T>(
    source: Result<&str, DataErrorCause>,
    path: &str,
    parse: impl FnOnce(&str) -> Result<T, ConfigError>,
) -> Result<T, DataLoadError> {
    source
        .and_then(|text| parse(text).map_err(DataErrorCause::from))
        .map_err(|cause| DataLoadError {
            path: path.into(),
            category: DataCategory::Rules,
            cause,
        })
}

/// Builds the validated library from already-read sources.
///
/// Split out of the polling system so the blocking-versus-narrowing scope is
/// testable without an asset server.
pub fn build_library(
    sources: &[Result<&str, DataErrorCause>],
    paths: &[&str],
    excluded_characters: &mut Vec<DataLoadError>,
) -> Result<ValidatedRuleLibrary, DataLoadError> {
    let profile = parse_blocking(sources[0].clone(), paths[0], parse_rule_profile)?;
    let roster = parse_blocking(sources[1].clone(), paths[1], parse_roster)?;
    let book = parse_blocking(sources[2].clone(), paths[2], parse_fever_puzzle_book)?;

    let mut plays = Vec::new();
    for (offset, source) in sources[3..].iter().enumerate() {
        match parse_blocking(source.clone(), paths[3 + offset], parse_character_play) {
            Ok(play) => plays.push(play),
            Err(error) => excluded_characters.push(error),
        }
    }

    let (library, report) = ValidatedRuleLibrary::partial(vec![profile], roster, plays, vec![book])
        .map_err(|error| DataLoadError {
            path: paths[0].into(),
            category: DataCategory::Rules,
            cause: error.into(),
        })?;
    for unavailable in report.unavailable_characters {
        warn!(
            "character {} is not selectable: {}",
            unavailable.character.0, unavailable.reason
        );
    }
    Ok(library)
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
    source: Result<&str, DataErrorCause>,
    parser: impl FnOnce(&str) -> Result<T, DataErrorCause>,
) -> DataResolution<T> {
    let path = path.as_ref();
    let outcome = source.and_then(parser);
    match outcome {
        Ok(value) => DataResolution::Loaded(value),
        Err(cause) => DataResolution::Failed(DataLoadError {
            path: path.into(),
            category,
            cause,
        }),
    }
}
