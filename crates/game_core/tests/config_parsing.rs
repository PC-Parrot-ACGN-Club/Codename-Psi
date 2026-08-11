//! In-memory RON/JSON parsing, schema gating, and typed error classification.

use game_core::config::{ConfigError, STUB_SCHEMA_VERSION, parse_i18n_stub, parse_rules_stub};

// docs/test/game-infrastructure.md TC-028
#[test]
fn a_supported_ron_document_parses_into_typed_data() {
    let source = r#"(
    schema_version: 1,
    id: "stub",
)
"#;

    let parsed = parse_rules_stub(source).expect("schema 1 parses");

    assert_eq!(parsed.schema_version, STUB_SCHEMA_VERSION);
    assert_eq!(parsed.id, "stub");
}

// docs/test/game-infrastructure.md TC-029
#[test]
fn a_truncated_ron_document_returns_a_parse_error_with_its_cause() {
    let error = parse_rules_stub("(schema_version: 1, id: ").expect_err("truncated RON fails");

    match error {
        ConfigError::Ron(reason) => assert!(
            !reason.is_empty(),
            "the typed error must keep the underlying reason"
        ),
        other => panic!("expected a RON parse error, got {other:?}"),
    }
}

// docs/test/game-infrastructure.md TC-029
#[test]
fn a_truncated_json_document_returns_a_parse_error_with_its_cause() {
    let error = parse_i18n_stub(r#"{"schema_version": 1, "messages": {"#)
        .expect_err("truncated JSON fails");

    match error {
        ConfigError::Json(reason) => assert!(
            !reason.is_empty(),
            "the typed error must keep the underlying reason"
        ),
        other => panic!("expected a JSON parse error, got {other:?}"),
    }
}

// docs/test/game-infrastructure.md TC-030
#[test]
fn an_unsupported_ron_schema_reports_the_actual_version() {
    let error = parse_rules_stub(r#"(schema_version: 255, id: "stub")"#)
        .expect_err("schema 255 is not supported");

    assert_eq!(
        error,
        ConfigError::UnsupportedSchema {
            found: 255,
            supported: STUB_SCHEMA_VERSION,
        }
    );
}

// docs/test/game-infrastructure.md TC-030
#[test]
fn an_unsupported_json_schema_reports_the_actual_version() {
    let error = parse_i18n_stub(r#"{"schema_version": 255, "messages": {}}"#)
        .expect_err("schema 255 is not supported");

    assert_eq!(
        error,
        ConfigError::UnsupportedSchema {
            found: 255,
            supported: STUB_SCHEMA_VERSION,
        }
    );
}
