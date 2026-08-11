//! Runtime data resolution and error-context types.

use std::path::{Path, PathBuf};

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

pub fn resolve_text<T>(
    path: impl AsRef<Path>,
    category: DataCategory,
    fallback: T,
    parser: impl FnOnce(&str) -> Result<T, DataErrorCause>,
) -> DataResolution<T> {
    let path = path.as_ref();
    match std::fs::read_to_string(path) {
        Ok(source) => match parser(&source) {
            Ok(value) => DataResolution::Loaded(value),
            Err(cause) => DataResolution::Fallback {
                value: fallback,
                error: DataLoadError {
                    path: path.into(),
                    category,
                    cause,
                },
            },
        },
        Err(error) => DataResolution::Fallback {
            value: fallback,
            error: DataLoadError {
                path: path.into(),
                category,
                cause: DataErrorCause::Io(error.to_string()),
            },
        },
    }
}
