//! Runtime data resolution and error-context types.
//!
//! Reading and parsing are deliberately separate. Bevy Asset owns reading, so
//! the client gets one asset pipeline and one asset root; parsing stays in
//! `game_core::config` and the client parsers, which is what keeps the four
//! typed causes (`Io`, `Parse`, `UnsupportedSchema`, `InvalidData`)
//! distinguishable. An `AssetLoader` that parsed as well would collapse the
//! last three into Bevy's opaque load failure.

use std::path::{Path, PathBuf};

use bevy::asset::io::Reader;
use bevy::asset::{Asset, AssetLoader, LoadContext};
use bevy::prelude::*;
use game_core::config::ConfigError;

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

/// Registers the project's asset reading path.
#[derive(Debug, Default)]
pub struct DataPlugin;

impl Plugin for DataPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SourceText>()
            .register_asset_loader(SourceTextLoader);
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
