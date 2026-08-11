//! Runtime data resolution and error-context types.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataCategory {
    Rules,
    Localization,
    Settings,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataErrorCause {
    Io(String),
    Parse(String),
    UnsupportedSchema { found: u32, supported: u32 },
    InvalidData(String),
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
    #[must_use]
    pub const fn is_resolved(&self) -> bool {
        true
    }

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
