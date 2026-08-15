//! The project root plugin's own data loading, seen from the consumer side.
//!
//! `data_loading.rs` proves the loader, parser and resolution helper cooperate,
//! but it assembles the lifecycle itself. Here nothing is assembled by the
//! test: the plugin has to request, poll and publish on its own, or no typed
//! data ever appears.

mod common;

use bevy::prelude::*;
use client::data::{DataCategory, DataErrorCause, RulesData};
use game_core::config::RuleProfileId;

use common::{WORKSPACE_ASSETS, controlled_app, controlled_app_with_asset_root};

/// Pump frames until the plugin publishes its resolution.
///
/// Reads are asynchronous, so a single update is not enough. The cap only
/// stops a broken build from hanging the suite; the production timeout is what
/// guarantees a resolution appears.
fn run_until_rules_resolved(app: &mut App) {
    for _ in 0..2000 {
        app.update();
        if app.world().get_resource::<RulesData>().is_some() {
            return;
        }
    }
    panic!("the rules load never resolved");
}

// integration-system/runtime-data::TC-003
#[test]
fn the_root_plugin_publishes_typed_rules_from_the_repository_assets() {
    let mut app = controlled_app();
    run_until_rules_resolved(&mut app);

    let data = app.world().resource::<RulesData>();
    assert_eq!(
        data.error(),
        None,
        "the repository's own rules file must load cleanly"
    );
    assert_eq!(
        data.rules()
            .expect("repository library is loaded")
            .profile(&RuleProfileId("fever-r1".into()))
            .expect("profile exists")
            .id
            .0,
        "fever-r1",
        "the consumer reads the parsed document, not the source text"
    );
    assert_eq!(
        data.rules()
            .expect("repository library is loaded")
            .profile(&RuleProfileId("fever-r1".into()))
            .expect("profile exists")
            .schema_version,
        1
    );
}

// integration-system/runtime-data::TC-003
#[test]
fn a_missing_rules_file_blocks_match_data_with_a_typed_error() {
    let root = tempfile::tempdir().expect("a temporary asset root");
    let mut app = controlled_app_with_asset_root(root.path().to_string_lossy().to_string());
    run_until_rules_resolved(&mut app);

    let data = app.world().resource::<RulesData>();
    let error = data
        .error()
        .expect("a missing file must resolve as a failure, not as a clean load");

    assert_eq!(error.category, DataCategory::Rules);
    assert!(
        matches!(error.cause, DataErrorCause::Io(_)),
        "an unreadable file is an Io cause, not a parse failure: {:?}",
        error.cause
    );
    assert!(
        error.path.ends_with("rules/profiles/fever.ron"),
        "the diagnostic keeps the resource path: {:?}",
        error.path
    );
    assert!(
        data.rules().is_none(),
        "rules failure must not synthesize authority"
    );
}

// integration-system/runtime-data::TC-003
#[test]
fn the_plugin_owns_the_path_so_consumers_never_name_it() {
    // The asset root is plugin configuration; pointing it at the workspace
    // tree is all a consumer ever does. Nothing here names the rules file.
    let mut app = controlled_app_with_asset_root(WORKSPACE_ASSETS);
    run_until_rules_resolved(&mut app);

    assert!(
        app.world()
            .get_resource::<client::data::RulesLoad>()
            .is_none(),
        "the in-flight read is dropped once the resolution is published"
    );
    assert_eq!(
        app.world()
            .resource::<RulesData>()
            .rules()
            .expect("library loaded")
            .profile(&RuleProfileId("fever-r1".into()))
            .expect("profile exists")
            .id
            .0,
        "fever-r1"
    );
}

/// Blocking scope: a profile failure stops every match, one character's
/// unusable gameplay data only removes that character.
///
/// Exercised through `build_library` rather than the asset server, so the
/// scope rule is tested independently of read timing.
// integration-system/runtime-data::TC-001
#[test]
fn a_blocking_failure_stops_the_match_while_a_character_failure_only_narrows_selection() {
    const PROFILE: &str = include_str!("../../../assets/data/rules/profiles/fever.ron");
    const ROSTER: &str = include_str!("../../../assets/data/rules/roster.ron");
    const BOOK: &str = include_str!("../../../assets/data/rules/puzzles/fever-r1.ron");
    const PLAY_A: &str = include_str!("../../../assets/data/rules/play/fever-r1/psi-a.ron");
    const PLAY_B: &str = include_str!("../../../assets/data/rules/play/fever-r1/psi-b.ron");

    let paths = client::data::rules_paths();

    // One unusable character file: the library still resolves, and the other
    // character stays selectable.
    let mut excluded = Vec::new();
    let library = client::data::build_library(
        &[
            Ok(PROFILE),
            Ok(ROSTER),
            Ok(BOOK),
            Ok(PLAY_A),
            Err(DataErrorCause::Parse("truncated".into())),
        ],
        &paths,
        &mut excluded,
    )
    .expect("one bad character must not block the profile");
    assert_eq!(excluded.len(), 1, "the failure is recorded, not swallowed");
    assert!(excluded[0].path.ends_with("psi-b.ron"));
    assert_eq!(excluded[0].category, DataCategory::Rules);
    assert!(
        library
            .character_play(
                &RuleProfileId("fever-r1".into()),
                &game_core::config::CharacterId("psi-a".into()),
            )
            .is_some(),
        "the healthy character is still playable"
    );

    // An unusable profile blocks everything, whatever the character files say.
    let mut excluded = Vec::new();
    let error = client::data::build_library(
        &[
            Err(DataErrorCause::Io("missing".into())),
            Ok(ROSTER),
            Ok(BOOK),
            Ok(PLAY_A),
            Ok(PLAY_B),
        ],
        &paths,
        &mut excluded,
    )
    .expect_err("a missing profile leaves no authority to play under");
    assert!(error.path.ends_with("fever.ron"));
    assert!(matches!(error.cause, DataErrorCause::Io(_)));
}

/// Pump frames until the plugin publishes the presentation resolution.
fn run_until_presentation_resolved(app: &mut App) {
    for _ in 0..2000 {
        app.update();
        if app
            .world()
            .get_resource::<client::data::CharacterPresentationData>()
            .is_some()
        {
            return;
        }
    }
    panic!("the presentation load never resolved");
}

// integration-system/runtime-data::TC-003
#[test]
fn the_root_plugin_publishes_the_character_presentation_catalog() {
    let mut app = controlled_app();
    run_until_presentation_resolved(&mut app);

    let data = app
        .world()
        .resource::<client::data::CharacterPresentationData>();
    let catalog = data
        .0
        .loaded()
        .expect("the repository's own presentation file must load cleanly");
    for id in ["psi-a", "psi-b"] {
        let entry = catalog
            .get(&game_core::config::CharacterId(id.into()))
            .unwrap_or_else(|| panic!("{id} is in the catalog"));
        assert_eq!(
            entry.poses.len(),
            client::character_presentation::POSES.len(),
            "{id} carries every pose the portraits can ask for"
        );
        assert!(!entry.badge.glyph.is_empty());
    }
}

// integration-system/runtime-data::TC-003
#[test]
fn a_missing_presentation_file_degrades_instead_of_blocking() {
    let root = tempfile::tempdir().expect("a temporary asset root");
    let mut app = controlled_app_with_asset_root(root.path().to_string_lossy().to_string());
    run_until_presentation_resolved(&mut app);

    let data = app
        .world()
        .resource::<client::data::CharacterPresentationData>();
    let error = data
        .0
        .error()
        .expect("a missing file must resolve as a failure, not as a clean load");
    assert_eq!(error.category, DataCategory::Presentation);
    assert!(matches!(error.cause, DataErrorCause::Io(_)));
}
