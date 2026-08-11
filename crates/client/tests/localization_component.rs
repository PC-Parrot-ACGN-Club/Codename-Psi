//! Locale selection, English fallback, key placeholders, and catalog validation.

use std::collections::BTreeMap;

use client::i18n::{
    CATALOG_SCHEMA_VERSION, Catalog, CatalogError, DEFAULT_LOCALE, Localization,
    MissingKeyDiagnostic, SUPPORTED_LOCALES, parse_catalog,
};

const ASSET_EN: &str = include_str!("../../../assets/i18n/en.json");
const ASSET_ZH: &str = include_str!("../../../assets/i18n/zh-CN.json");

fn catalog(locale: &str, entries: &[(&str, &str)]) -> Catalog {
    Catalog {
        schema_version: CATALOG_SCHEMA_VERSION,
        locale: locale.into(),
        messages: entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect(),
    }
}

// docs/test/game-infrastructure.md TC-007
#[test]
fn localization_defaults_to_english_without_a_valid_language_setting() {
    let localization = Localization::default();

    assert_eq!(localization.current_locale, DEFAULT_LOCALE);
    assert_eq!(localization.text("main_menu.start"), "Start");
}

// docs/test/game-infrastructure.md TC-008
#[test]
fn a_key_present_in_the_current_locale_is_returned_directly() {
    let localization = Localization::new(
        "zh-CN",
        [
            catalog("zh-CN", &[("main_menu.start", "开始")]),
            catalog("en", &[("main_menu.start", "Start")]),
        ],
    );

    let text = localization.text("main_menu.start");

    assert_eq!(text, "开始");
    assert!(
        localization.diagnostics().is_empty(),
        "a present key must not raise a missing-key diagnostic"
    );
}

// docs/test/game-infrastructure.md TC-009
#[test]
fn a_key_missing_from_the_current_locale_falls_back_to_english_with_a_diagnostic() {
    let localization = Localization::new(
        "zh-CN",
        [
            catalog("zh-CN", &[("main_menu.start", "开始")]),
            catalog("en", &[("main_menu.settings", "Settings")]),
        ],
    );

    let text = localization.text("main_menu.settings");

    assert_eq!(text, "Settings");
    assert_eq!(
        localization.diagnostics(),
        vec![MissingKeyDiagnostic {
            locale: "zh-CN".into(),
            key: "main_menu.settings".into(),
        }]
    );
}

// docs/test/game-infrastructure.md TC-010
#[test]
fn a_key_missing_everywhere_is_returned_as_its_own_placeholder() {
    let localization = Localization::new("zh-CN", [catalog("zh-CN", &[]), catalog("en", &[])]);

    let text = localization.text("missing.example");

    assert_eq!(text, "missing.example");
    assert_eq!(
        localization.diagnostics(),
        vec![MissingKeyDiagnostic {
            locale: "zh-CN".into(),
            key: "missing.example".into(),
        }]
    );
}

// docs/test/game-infrastructure.md TC-011
#[test]
fn switching_the_locale_redirects_later_queries_to_the_new_catalog() {
    let mut localization = Localization::new(
        "en",
        [
            catalog("zh-CN", &[("main_menu.start", "开始")]),
            catalog("en", &[("main_menu.start", "Start")]),
        ],
    );

    assert_eq!(localization.text("main_menu.start"), "Start");

    localization.set_locale("zh-CN");

    let read_only: &Localization = &localization;
    assert_eq!(read_only.text("main_menu.start"), "开始");
    assert!(read_only.diagnostics().is_empty());
}

// docs/test/game-infrastructure.md TC-012
#[test]
fn a_valid_catalog_parses_into_a_locale_and_messages() {
    let source = r#"{
        "schema_version": 1,
        "locale": "en",
        "messages": { "main_menu.start": "Start" }
    }"#;

    let parsed = parse_catalog(source).expect("schema 1 with a supported locale parses");

    assert_eq!(parsed.schema_version, CATALOG_SCHEMA_VERSION);
    assert_eq!(parsed.locale, "en");
    assert_eq!(
        parsed.messages,
        BTreeMap::from([("main_menu.start".to_string(), "Start".to_string())])
    );
}

// docs/test/game-infrastructure.md TC-012
#[test]
fn the_project_catalogs_build_from_their_json_assets() {
    let en = parse_catalog(ASSET_EN).expect("assets/i18n/en.json parses");
    let zh = parse_catalog(ASSET_ZH).expect("assets/i18n/zh-CN.json parses");

    assert_eq!(en.locale, "en");
    assert_eq!(zh.locale, "zh-CN");
    assert_eq!(en.messages.get("main_menu.start").unwrap(), "Start");
    assert_eq!(zh.messages.get("main_menu.start").unwrap(), "开始");
}

// docs/test/game-infrastructure.md TC-012, TC-029
#[test]
fn a_truncated_catalog_returns_a_parse_error() {
    let error = parse_catalog(r#"{"schema_version": 1, "locale": "en", "messages": {"#)
        .expect_err("a truncated document cannot parse");

    match error {
        CatalogError::Parse(reason) => assert!(
            !reason.is_empty(),
            "the parse error must keep the underlying reason"
        ),
        other => panic!("expected a parse error, got {other:?}"),
    }
}

// docs/test/game-infrastructure.md TC-012, TC-030
#[test]
fn an_unsupported_catalog_schema_reports_the_actual_version() {
    let source = r#"{ "schema_version": 255, "locale": "en", "messages": {} }"#;

    let error = parse_catalog(source).expect_err("schema 255 is not supported");

    assert_eq!(
        error,
        CatalogError::UnsupportedSchema {
            found: 255,
            supported: CATALOG_SCHEMA_VERSION,
        }
    );
}

// docs/test/game-infrastructure.md TC-031
#[test]
fn a_catalog_locale_outside_the_supported_set_returns_invalid_data() {
    let source = r#"{ "schema_version": 1, "locale": "fr", "messages": {} }"#;

    let error = parse_catalog(source).expect_err("fr is outside the supported locale set");

    assert_eq!(
        error,
        CatalogError::InvalidData {
            found: "fr".into(),
            supported: &SUPPORTED_LOCALES,
        },
        "the error must name the violated constraint and keep the actual locale"
    );
}
