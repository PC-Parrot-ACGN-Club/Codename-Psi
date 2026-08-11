//! Resource loading through a minimal Bevy app: `Loaded` plus four failure kinds.
//!
//! Reading stays on `std::fs` by decision; the Bevy app supplies the runtime the
//! resolution actually happens in, and the asset root is a temporary directory.

use std::path::PathBuf;

use bevy::prelude::*;
use client::data::{DataCategory, DataErrorCause, DataResolution, resolve_text};
use client::i18n::{Catalog, builtin_english_catalog, parse_catalog};
use game_core::config::{RulesStub, parse_rules_stub};

#[derive(Debug, Resource)]
struct AssetRoot(PathBuf);

#[derive(Debug, Resource)]
struct RulesResolutions(Vec<(&'static str, DataResolution<RulesStub>)>);

#[derive(Debug, Resource)]
struct CatalogResolution(DataResolution<Catalog>);

fn builtin_rules_default() -> RulesStub {
    RulesStub {
        schema_version: 1,
        id: "builtin-default".into(),
    }
}

fn resolve_fixtures(root: Res<AssetRoot>, mut commands: Commands) {
    let data = root.0.join("data");
    let rules = ["valid", "missing", "malformed", "unsupported"]
        .into_iter()
        .map(|name| {
            let resolution = resolve_text(
                data.join(format!("rules.{name}.ron")),
                DataCategory::Rules,
                builtin_rules_default(),
                |source| parse_rules_stub(source).map_err(DataErrorCause::from),
            );
            (name, resolution)
        })
        .collect();
    commands.insert_resource(RulesResolutions(rules));

    let catalog = resolve_text(
        root.0.join("i18n").join("invalid.json"),
        DataCategory::Localization,
        builtin_english_catalog(),
        |source| parse_catalog(source).map_err(DataErrorCause::from),
    );
    commands.insert_resource(CatalogResolution(catalog));
}

/// A temporary asset root holding one fixture per resolution outcome.
fn app_with_fixtures() -> (App, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("temporary asset root");
    let data = root.path().join("data");
    let i18n = root.path().join("i18n");
    std::fs::create_dir_all(&data).expect("create data dir");
    std::fs::create_dir_all(&i18n).expect("create i18n dir");

    std::fs::write(
        data.join("rules.valid.ron"),
        "(\n    schema_version: 1,\n    id: \"stub\",\n)\n",
    )
    .expect("write valid rules");
    std::fs::write(data.join("rules.malformed.ron"), "(schema_version: 1, id: ")
        .expect("write malformed rules");
    std::fs::write(
        data.join("rules.unsupported.ron"),
        "(schema_version: 255, id: \"stub\")",
    )
    .expect("write unsupported rules");
    // rules.missing.ron is intentionally never created.
    std::fs::write(
        i18n.join("invalid.json"),
        r#"{ "schema_version": 1, "locale": "fr", "messages": {} }"#,
    )
    .expect("write invalid catalog");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(AssetRoot(root.path().to_path_buf()))
        .add_systems(Startup, resolve_fixtures);
    app.update();

    (app, root)
}

fn rules_resolution(app: &App, name: &str) -> DataResolution<RulesStub> {
    app.world()
        .resource::<RulesResolutions>()
        .0
        .iter()
        .find(|(fixture, _)| *fixture == name)
        .map(|(_, resolution)| resolution.clone())
        .unwrap_or_else(|| panic!("fixture {name} was resolved"))
}

// docs/test/game-infrastructure.md TC-032
#[test]
fn a_valid_resource_resolves_to_loaded_typed_data() {
    let (app, _root) = app_with_fixtures();

    let resolution = rules_resolution(&app, "valid");

    assert!(resolution.is_resolved());
    assert_eq!(resolution.error(), None);
    assert_eq!(resolution.value().id, "stub");
    assert!(matches!(resolution, DataResolution::Loaded(_)));
}

// docs/test/game-infrastructure.md TC-032
#[test]
fn a_missing_resource_falls_back_with_io_context() {
    let (app, _root) = app_with_fixtures();

    let resolution = rules_resolution(&app, "missing");

    let error = resolution.error().expect("a missing file keeps its error");
    assert!(matches!(error.cause, DataErrorCause::Io(_)));
    assert_eq!(error.category, DataCategory::Rules);
    assert!(error.path.ends_with("rules.missing.ron"));
    assert_eq!(resolution.value(), &builtin_rules_default());
    assert!(resolution.is_resolved());
}

// docs/test/game-infrastructure.md TC-032
#[test]
fn a_malformed_resource_falls_back_with_a_parse_cause() {
    let (app, _root) = app_with_fixtures();

    let resolution = rules_resolution(&app, "malformed");

    let error = resolution
        .error()
        .expect("a malformed file keeps its error");
    assert!(matches!(error.cause, DataErrorCause::Parse(_)));
    assert_eq!(error.category, DataCategory::Rules);
    assert!(error.path.ends_with("rules.malformed.ron"));
    assert_eq!(resolution.value(), &builtin_rules_default());
}

// docs/test/game-infrastructure.md TC-032
#[test]
fn an_unsupported_resource_falls_back_while_keeping_the_version() {
    let (app, _root) = app_with_fixtures();

    let resolution = rules_resolution(&app, "unsupported");

    let error = resolution
        .error()
        .expect("an unsupported file keeps its error");
    assert_eq!(
        error.cause,
        DataErrorCause::UnsupportedSchema {
            found: 255,
            supported: 1,
        }
    );
    assert_eq!(resolution.value(), &builtin_rules_default());
}

// docs/test/game-infrastructure.md TC-032
#[test]
fn a_semantically_invalid_catalog_falls_back_with_an_invalid_data_cause() {
    let (app, _root) = app_with_fixtures();

    let resolution = app.world().resource::<CatalogResolution>().0.clone();

    let error = resolution
        .error()
        .expect("an invalid catalog keeps its error");
    assert_eq!(error.category, DataCategory::Localization);
    assert!(error.path.ends_with("invalid.json"));
    match &error.cause {
        DataErrorCause::InvalidData(reason) => assert!(
            reason.contains("fr"),
            "the typed cause must keep the actual locale, got {reason}"
        ),
        other => panic!("expected InvalidData, got {other:?}"),
    }
    assert_eq!(resolution.value(), &builtin_english_catalog());
    assert!(
        resolution.is_resolved(),
        "a fallback result is still resolved for consumers"
    );
}
