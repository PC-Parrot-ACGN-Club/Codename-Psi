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
    CharacterId, ConfigError, RuleProfileId, ValidatedRuleLibrary, parse_character_play,
    parse_fever_puzzle_book, parse_roster, parse_rule_profile,
};

use crate::character_presentation::CharacterPresentationCatalog;
use crate::i18n::CatalogError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Which `assets/` data class a load belongs to.
///
/// User settings are not listed: they live in the platform config directory,
/// not under `assets/`, and report failures as `SettingsError`.
pub enum DataCategory {
    Rules,
    Localization,
    /// Client-side character colours, badges, poses and audio keys.
    Presentation,
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
/// Where the client-side character presentation data lives under `assets/`.
pub const PRESENTATION_CHARACTERS_PATH: &str = "data/presentation/characters.ron";

/// The rules documents that can be requested before anything has parsed:
/// profile, roster, puzzle book, in that order.
#[must_use]
pub const fn core_rules_paths() -> [&'static str; 3] {
    [
        RULES_PROFILE_PATH,
        RULES_ROSTER_PATH,
        RULES_PUZZLE_BOOK_PATH,
    ]
}

/// Where one character's gameplay data lives under `assets/`.
///
/// The convention is `data/rules/play/<profile_id>/<character_id>.ron`
/// (`assets/README.md`). Nothing here names a character: the set of play files
/// is whatever the roster lists, which is why they cannot be requested
/// alongside the core documents — see [`poll_rules`].
#[must_use]
pub fn play_path(profile: &RuleProfileId, character: &CharacterId) -> String {
    format!("data/rules/play/{}/{}.ron", profile.0, character.0)
}

/// How long a data read may hang before it is treated as failed.
pub const DATA_LOAD_TIMEOUT: Duration = Duration::from_secs(5);

/// A rules read in flight; removed once the resolution is published.
///
/// `waited` spans both stages, so adding the second read does not widen the
/// window the `Boot` barrier can be held open for.
#[derive(Debug, Resource)]
pub struct RulesLoad {
    stage: RulesStage,
    waited: Duration,
}

/// Which of the two reads is outstanding.
#[derive(Debug)]
enum RulesStage {
    /// The documents of [`core_rules_paths`], in that order.
    Core(Vec<Handle<SourceText>>),
    /// The core text held for the final build, plus one read per rostered
    /// character.
    Plays {
        core: Vec<Result<String, DataErrorCause>>,
        paths: Vec<String>,
        handles: Vec<Handle<SourceText>>,
    },
}

impl RulesStage {
    /// The handles this stage is waiting on.
    fn handles(&self) -> &[Handle<SourceText>] {
        match self {
            Self::Core(handles) | Self::Plays { handles, .. } => handles,
        }
    }
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

/// Ask Bevy Asset for the rules documents that can be named up front.
pub fn start_rules_load(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.insert_resource(RulesLoad {
        stage: RulesStage::Core(
            core_rules_paths()
                .into_iter()
                .map(|path| asset_server.load::<SourceText>(path))
                .collect(),
        ),
        waited: Duration::ZERO,
    });
}

/// Publish the rules resolution once the reads settle, or on timeout.
///
/// The read takes two stages because the roster is what says which play files
/// exist: the core documents are requested up front, and one play file per
/// rostered character only once the roster has parsed. Both stages share one
/// timeout budget, so the second read does not widen the `Boot` barrier.
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

    let Some(texts) = settle(load.stage.handles(), &asset_server, &sources, timed_out) else {
        return;
    };

    match std::mem::replace(&mut load.stage, RulesStage::Core(Vec::new())) {
        RulesStage::Core(_) => {
            let derived = rostered_play_paths(&borrowed(&texts));
            let Some(paths) = derived else {
                // Nothing to derive the play files from. The same core failure
                // is what `build_library` reports, so publish it now rather
                // than waiting out a second read that cannot be requested.
                publish(&mut commands, &borrowed(&texts), &core_rules_paths());
                return;
            };
            let handles = paths
                .iter()
                .map(|path| asset_server.load::<SourceText>(path.clone()))
                .collect();
            load.stage = RulesStage::Plays {
                core: texts,
                paths,
                handles,
            };
        }
        RulesStage::Plays { core, paths, .. } => {
            let mut all = core;
            all.extend(texts);
            let mut flat: Vec<&str> = core_rules_paths().to_vec();
            flat.extend(paths.iter().map(String::as_str));
            publish(&mut commands, &borrowed(&all), &flat);
        }
    }
}

/// Reads every handle a stage waits on, once all of them have settled.
///
/// `None` means at least one read is still in flight — the only state in which
/// this system leaves the resolution unpublished.
fn settle(
    handles: &[Handle<SourceText>],
    asset_server: &AssetServer,
    sources: &Assets<SourceText>,
    timed_out: bool,
) -> Option<Vec<Result<String, DataErrorCause>>> {
    let mut texts = Vec::with_capacity(handles.len());
    for handle in handles {
        texts.push(match asset_server.load_state(handle) {
            LoadState::Loaded => sources
                .get(handle)
                .map(|text| Ok(text.0.clone()))
                .unwrap_or(Err(DataErrorCause::Io("asset dropped after load".into()))),
            LoadState::Failed(error) => Err(DataErrorCause::Io(error.to_string())),
            _ if timed_out => Err(DataErrorCause::Io(format!(
                "timed out after {}s",
                DATA_LOAD_TIMEOUT.as_secs()
            ))),
            _ => return None,
        });
    }
    Some(texts)
}

/// Borrows settled text in the shape the builder reads.
fn borrowed(texts: &[Result<String, DataErrorCause>]) -> Vec<Result<&str, DataErrorCause>> {
    texts
        .iter()
        .map(|text| text.as_deref().map_err(Clone::clone))
        .collect()
}

/// The play files this roster asks for, or `None` when the core cannot say.
///
/// Only the profile and the roster are parsed here — the profile names the
/// directory and the roster names the files. The puzzle book is left to the
/// final build, so every blocking failure is still reported from one place.
fn rostered_play_paths(core: &[Result<&str, DataErrorCause>]) -> Option<Vec<String>> {
    let paths = core_rules_paths();
    let profile = parse_blocking(core[0].clone(), paths[0], parse_rule_profile).ok()?;
    let roster = parse_blocking(core[1].clone(), paths[1], parse_roster).ok()?;
    Some(
        roster
            .characters
            .iter()
            .map(|identity| play_path(&profile.id, &identity.id))
            .collect(),
    )
}

/// Build the library from settled text and hand the resolution to the app.
fn publish(commands: &mut Commands, sources: &[Result<&str, DataErrorCause>], paths: &[&str]) {
    let mut excluded_characters = Vec::new();
    let resolution = match build_library(sources, paths, &mut excluded_characters) {
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
/// `sources` and `paths` are the three core documents of [`core_rules_paths`]
/// in that order, followed by one play file per rostered character.
///
/// Blocking scope: the profile, roster and puzzle book gate every match, so a
/// failure in any of them fails the whole resolution. A character's gameplay
/// data only gates that character. Split out of the polling system so that
/// scope is testable without an asset server.
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

/// A presentation read in flight; removed once the resolution is published.
#[derive(Debug, Resource)]
pub struct PresentationLoad {
    handle: Handle<SourceText>,
    waited: Duration,
}

/// The resolved character presentation catalog.
///
/// Degradable scope: a failure blocks nothing and narrows nothing. Characters
/// stay selectable and matches stay playable; they only lose the colours,
/// badges and poses that tell them apart, and the resolver hands out its
/// per-slot substitute instead.
#[derive(Debug, Resource)]
pub struct CharacterPresentationData(pub DataResolution<CharacterPresentationCatalog>);

/// Ask Bevy Asset for the character presentation document.
pub fn start_presentation_load(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.insert_resource(PresentationLoad {
        handle: asset_server.load::<SourceText>(PRESENTATION_CHARACTERS_PATH),
        waited: Duration::ZERO,
    });
}

/// Publish the presentation resolution once the read and the roster settle.
///
/// The roster is what the catalog is validated against, so this waits for the
/// rules resolution as well. Rules that failed leave nothing to validate
/// against, and the catalog resolves to that same failure's substitute.
pub fn poll_presentation(
    time: Res<Time<Real>>,
    asset_server: Res<AssetServer>,
    sources: Res<Assets<SourceText>>,
    rules: Option<Res<RulesData>>,
    mut load: ResMut<PresentationLoad>,
    mut commands: Commands,
) {
    load.waited += time.delta();
    let timed_out = load.waited >= DATA_LOAD_TIMEOUT;
    let Some(rules) = rules else {
        return;
    };

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
        _ => return,
    };

    let resolution = match rules.rules() {
        Some(library) => {
            let roster = library.roster().clone();
            resolve_source(
                PRESENTATION_CHARACTERS_PATH,
                DataCategory::Presentation,
                source,
                |text| crate::character_presentation::parse_character_presentations(text, &roster),
            )
        }
        None => DataResolution::Failed(DataLoadError {
            path: PRESENTATION_CHARACTERS_PATH.into(),
            category: DataCategory::Presentation,
            cause: DataErrorCause::Io("rules unavailable, so nothing to validate against".into()),
        }),
    };
    if let DataResolution::Failed(error) = &resolution {
        warn!("character presentation falls back to substitutes: {error:?}");
    }
    commands.insert_resource(CharacterPresentationData(resolution));
    commands.remove_resource::<PresentationLoad>();
}

/// Registers the project's asset reading path and data load lifecycle.
#[derive(Debug, Default)]
pub struct DataPlugin;

impl Plugin for DataPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SourceText>()
            .register_asset_loader(SourceTextLoader)
            .add_systems(Startup, (start_rules_load, start_presentation_load))
            .add_systems(
                Update,
                (
                    poll_rules.run_if(resource_exists::<RulesLoad>),
                    poll_presentation.run_if(resource_exists::<PresentationLoad>),
                ),
            );
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
