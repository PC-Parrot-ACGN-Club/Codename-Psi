//! JSON localization catalogs with English and key fallback seams.

use std::collections::BTreeMap;
use std::sync::Mutex;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CATALOG_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_LOCALE: &str = "en";
pub const SUPPORTED_LOCALES: [&str; 2] = ["zh-CN", "en"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub schema_version: u32,
    pub locale: String,
    pub messages: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CatalogError {
    #[error("catalog JSON parse failed: {0}")]
    Parse(String),
    #[error("unsupported catalog schema {found} (supported: {supported})")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("unsupported catalog locale {found}; expected one of {supported:?}")]
    InvalidData {
        found: String,
        supported: &'static [&'static str],
    },
}

pub fn parse_catalog(source: &str) -> Result<Catalog, CatalogError> {
    let catalog: Catalog =
        serde_json::from_str(source).map_err(|error| CatalogError::Parse(error.to_string()))?;
    if catalog.schema_version != CATALOG_SCHEMA_VERSION {
        return Err(CatalogError::UnsupportedSchema {
            found: catalog.schema_version,
            supported: CATALOG_SCHEMA_VERSION,
        });
    }
    if !SUPPORTED_LOCALES.contains(&catalog.locale.as_str()) {
        return Err(CatalogError::InvalidData {
            found: catalog.locale,
            supported: &SUPPORTED_LOCALES,
        });
    }
    Ok(catalog)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingKeyDiagnostic {
    pub locale: String,
    pub key: String,
}

#[derive(Debug, Resource)]
pub struct Localization {
    pub current_locale: String,
    pub catalogs: BTreeMap<String, Catalog>,
    diagnostics: Mutex<Vec<MissingKeyDiagnostic>>,
}

impl Default for Localization {
    fn default() -> Self {
        Self::new(DEFAULT_LOCALE, [builtin_english_catalog()])
    }
}

impl Localization {
    #[must_use]
    pub fn new(locale: impl Into<String>, catalogs: impl IntoIterator<Item = Catalog>) -> Self {
        Self {
            current_locale: locale.into(),
            catalogs: catalogs
                .into_iter()
                .map(|catalog| (catalog.locale.clone(), catalog))
                .collect(),
            diagnostics: Mutex::new(Vec::new()),
        }
    }

    /// Switch the current locale, falling back when it has no catalog.
    ///
    /// Returns whether the requested locale was available. An unavailable
    /// locale leaves every lookup falling through to English anyway, so this
    /// selects the default outright rather than leaving the resource pointing
    /// at a catalog that does not exist.
    pub fn set_locale(&mut self, locale: impl Into<String>) -> bool {
        let locale = locale.into();
        let available = self.catalogs.contains_key(&locale);
        self.current_locale = if available {
            locale
        } else {
            DEFAULT_LOCALE.to_string()
        };
        available
    }

    #[must_use]
    pub fn text(&self, key: &str) -> String {
        if let Some(value) = self
            .catalogs
            .get(&self.current_locale)
            .and_then(|catalog| catalog.messages.get(key))
        {
            return value.clone();
        }

        // UI code queries text every frame, so an unfiltered log would grow
        // without bound on a single missing key. One entry per (locale, key)
        // keeps the diagnostic just as informative and bounds it by the size
        // of the catalog.
        let diagnostic = MissingKeyDiagnostic {
            locale: self.current_locale.clone(),
            key: key.into(),
        };
        let mut diagnostics = self
            .diagnostics
            .lock()
            .expect("localization diagnostic mutex poisoned");
        if !diagnostics.contains(&diagnostic) {
            diagnostics.push(diagnostic);
        }
        drop(diagnostics);

        self.catalogs
            .get(DEFAULT_LOCALE)
            .and_then(|catalog| catalog.messages.get(key))
            .cloned()
            .unwrap_or_else(|| key.into())
    }

    #[must_use]
    pub fn diagnostics(&self) -> Vec<MissingKeyDiagnostic> {
        self.diagnostics
            .lock()
            .expect("localization diagnostic mutex poisoned")
            .clone()
    }
}

#[derive(Debug, Default)]
pub struct LocalizationPlugin;

impl Plugin for LocalizationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Localization>();
    }
}

#[must_use]
pub fn builtin_english_catalog() -> Catalog {
    Catalog {
        schema_version: CATALOG_SCHEMA_VERSION,
        locale: DEFAULT_LOCALE.into(),
        messages: BTreeMap::from([
            ("app.title".into(), "Codename Psi".into()),
            ("main_menu.start".into(), "Start".into()),
            ("main_menu.settings".into(), "Settings".into()),
        ]),
    }
}
