//! Versioned config stubs parsed from in-memory RON / JSON.
//!
//! Full rule profiles replace these stubs when the deterministic kernel lands.

use std::collections::BTreeMap;

use serde::Deserialize;
use thiserror::Error;

/// Schema version supported by the current stub loaders.
pub const STUB_SCHEMA_VERSION: u32 = 1;

/// Minimal rules document used to prove the data loading path.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RulesStub {
    pub schema_version: u32,
    pub id: String,
}

/// Minimal localization table used to prove the i18n loading path.
///
/// Locale-set semantics belong to `client::i18n`; this stub only proves that a
/// versioned JSON document parses into typed data from an in-memory source.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct I18nStub {
    pub schema_version: u32,
    pub messages: BTreeMap<String, String>,
}

/// Typed failures for stub config parsing (development diagnostics).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("failed to parse RON rules stub: {0}")]
    Ron(String),
    #[error("failed to parse JSON i18n stub: {0}")]
    Json(String),
    #[error("unsupported schema_version {found} (supported: {supported})")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("invalid data: {0}")]
    InvalidData(String),
}

/// Parse a rules stub from a RON document string.
pub fn parse_rules_stub(ron_source: &str) -> Result<RulesStub, ConfigError> {
    let stub: RulesStub =
        ron::from_str(ron_source).map_err(|err| ConfigError::Ron(err.to_string()))?;
    ensure_schema(stub.schema_version)?;
    Ok(stub)
}

/// Parse an i18n stub from a JSON document string.
pub fn parse_i18n_stub(json_source: &str) -> Result<I18nStub, ConfigError> {
    let stub: I18nStub =
        serde_json::from_str(json_source).map_err(|err| ConfigError::Json(err.to_string()))?;
    ensure_schema(stub.schema_version)?;
    Ok(stub)
}

fn ensure_schema(found: u32) -> Result<(), ConfigError> {
    if found == STUB_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ConfigError::UnsupportedSchema {
            found,
            supported: STUB_SCHEMA_VERSION,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES_STUB: &str = include_str!("../../../assets/data/rules.stub.ron");
    const I18N_EN: &str = include_str!("../../../assets/i18n/en.json");
    const I18N_ZH: &str = include_str!("../../../assets/i18n/zh-CN.json");

    #[test]
    fn parses_rules_stub_from_assets() {
        let stub = parse_rules_stub(RULES_STUB).expect("rules stub should parse");
        assert_eq!(stub.schema_version, STUB_SCHEMA_VERSION);
        assert_eq!(stub.id, "stub");
    }

    #[test]
    fn parses_i18n_stubs_from_assets() {
        let en = parse_i18n_stub(I18N_EN).expect("en i18n stub should parse");
        let zh = parse_i18n_stub(I18N_ZH).expect("zh-CN i18n stub should parse");
        assert_eq!(en.schema_version, STUB_SCHEMA_VERSION);
        assert_eq!(zh.schema_version, STUB_SCHEMA_VERSION);
        assert_eq!(
            en.messages.get("app.title").map(String::as_str),
            Some("Codename Psi")
        );
        assert_eq!(
            zh.messages.get("app.title").map(String::as_str),
            Some("Codename Psi")
        );
    }

    #[test]
    fn rejects_unsupported_rules_schema() {
        let err = parse_rules_stub("(schema_version: 99, id: \"x\")\n")
            .expect_err("schema 99 should be rejected");
        assert_eq!(
            err,
            ConfigError::UnsupportedSchema {
                found: 99,
                supported: STUB_SCHEMA_VERSION,
            }
        );
    }

    #[test]
    fn rejects_invalid_ron() {
        let err = parse_rules_stub("not ron").expect_err("invalid ron should fail");
        assert!(matches!(err, ConfigError::Ron(_)));
    }

    #[test]
    fn rejects_invalid_json() {
        let err = parse_i18n_stub("{").expect_err("invalid json should fail");
        assert!(matches!(err, ConfigError::Json(_)));
    }
}
