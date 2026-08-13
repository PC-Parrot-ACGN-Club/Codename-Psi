//! Resource loading through Bevy Asset: `Loaded` plus four failure kinds.
//!
//! Reads go through the asset server against a temporary asset root, which is
//! the reading boundary the loading contract defines. Parsing stays outside the
//! asset loader so each failure keeps its typed cause.

use bevy::asset::{AssetPlugin, AssetServer, Assets, Handle, LoadState};
use bevy::prelude::*;
use client::data::{
    DataCategory, DataErrorCause, DataPlugin, DataResolution, SourceText, resolve_source,
};
use client::i18n::{Catalog, builtin_english_catalog, parse_catalog};
use game_core::config::{RulesStub, parse_rules_stub};

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

/// Handles requested in `Startup`, resolved once the reads settle.
#[derive(Debug, Resource)]
struct Pending {
    rules: Vec<(&'static str, Handle<SourceText>)>,
    catalog: Handle<SourceText>,
}

fn request_fixtures(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.insert_resource(Pending {
        rules: ["valid", "missing", "malformed", "unsupported"]
            .into_iter()
            .map(|name| (name, asset_server.load(format!("data/rules.{name}.ron"))))
            .collect(),
        catalog: asset_server.load("i18n/invalid.json"),
    });
}

/// Map a settled handle to the source text, or to the read failure.
fn source_of<'a>(
    asset_server: &AssetServer,
    sources: &'a Assets<SourceText>,
    handle: &Handle<SourceText>,
) -> Option<Result<&'a str, DataErrorCause>> {
    match asset_server.load_state(handle) {
        LoadState::Loaded => Some(
            sources
                .get(handle)
                .map(|text| Ok(text.0.as_str()))
                .unwrap_or(Err(DataErrorCause::Io("asset dropped".into()))),
        ),
        LoadState::Failed(error) => Some(Err(DataErrorCause::Io(error.to_string()))),
        _ => None,
    }
}

fn resolve_fixtures(
    asset_server: Res<AssetServer>,
    sources: Res<Assets<SourceText>>,
    pending: Res<Pending>,
    mut commands: Commands,
) {
    let mut rules = Vec::new();
    for (name, handle) in &pending.rules {
        let Some(source) = source_of(&asset_server, &sources, handle) else {
            return;
        };
        rules.push((
            *name,
            resolve_source(
                format!("data/rules.{name}.ron"),
                DataCategory::Rules,
                builtin_rules_default(),
                source,
                |text| parse_rules_stub(text).map_err(DataErrorCause::from),
            ),
        ));
    }

    let Some(source) = source_of(&asset_server, &sources, &pending.catalog) else {
        return;
    };
    let catalog = resolve_source(
        "i18n/invalid.json",
        DataCategory::Localization,
        builtin_english_catalog(),
        source,
        |text| parse_catalog(text).map_err(DataErrorCause::from),
    );

    commands.insert_resource(RulesResolutions(rules));
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
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: root.path().to_string_lossy().into_owned(),
            ..default()
        },
        DataPlugin,
    ))
    .add_systems(Startup, request_fixtures)
    .add_systems(Update, resolve_fixtures);

    // Reads are asynchronous; pump until every fixture has settled.
    for _ in 0..2000 {
        app.update();
        if app.world().get_resource::<CatalogResolution>().is_some() {
            return (app, root);
        }
    }
    panic!("fixtures never settled");
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

// integration-system/runtime-data::TC-001
#[test]
fn a_valid_resource_resolves_to_loaded_typed_data() {
    let (app, _root) = app_with_fixtures();

    let resolution = rules_resolution(&app, "valid");

    assert_eq!(resolution.error(), None);
    assert_eq!(resolution.value().id, "stub");
    assert!(matches!(resolution, DataResolution::Loaded(_)));
}

// integration-system/runtime-data::TC-001
#[test]
fn a_missing_resource_falls_back_with_io_context() {
    let (app, _root) = app_with_fixtures();

    let resolution = rules_resolution(&app, "missing");

    let error = resolution.error().expect("a missing file keeps its error");
    assert!(matches!(error.cause, DataErrorCause::Io(_)));
    assert_eq!(error.category, DataCategory::Rules);
    assert!(error.path.ends_with("rules.missing.ron"));
    assert_eq!(resolution.value(), &builtin_rules_default());
}

// integration-system/runtime-data::TC-001
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

// integration-system/runtime-data::TC-001
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

// integration-system/runtime-data::TC-001
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
}
