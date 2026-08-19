//! Character presentation data coverage from `component/character-presentation.md`.

use std::path::PathBuf;

use client::character_presentation::{
    AUDIO_CUES, CharacterPresentationCatalog, CharacterPresentationResolver, POSES,
    parse_character_presentations,
};
use client::data::{DataCategory, DataErrorCause, DataLoadError, DataResolution};
use game_core::config::{CharacterId, Roster, parse_roster};

const ROSTER: &str = include_str!("../../../assets/data/rules/roster.ron");
const PRESENTATION: &str = include_str!("../../../assets/data/presentation/characters.ron");

fn roster() -> Roster {
    parse_roster(ROSTER).expect("repository roster parses")
}

// component/character-presentation::TC-001
#[test]
fn repository_catalog_is_complete_and_indexed_by_character_id() {
    let catalog = parse_character_presentations(PRESENTATION, &roster()).expect("catalog parses");
    let a = catalog
        .get(&CharacterId("alpha".into()))
        .expect("alpha exists");
    let b = catalog
        .get(&CharacterId("beta".into()))
        .expect("beta exists");

    assert_eq!(a.poses.len(), POSES.len());
    assert_eq!(a.audio.len(), AUDIO_CUES.len());
    assert_eq!(b.poses.len(), POSES.len());
    assert_eq!(b.audio.len(), AUDIO_CUES.len());
    assert_ne!(a.primary_color, b.primary_color);
}

fn replace_once(source: &str, from: &str, to: &str) -> String {
    source.replacen(from, to, 1)
}

// component/character-presentation::TC-002
#[test]
fn unknown_character_is_invalid_data_with_the_id_preserved() {
    let source = replace_once(PRESENTATION, "id: (\"alpha\")", "id: (\"gamma\")");
    let error = parse_character_presentations(&source, &roster()).expect_err("unknown id fails");
    assert!(
        matches!(error, DataErrorCause::InvalidData(ref reason) if reason.contains("gamma") && reason.contains("roster"))
    );
}

// component/character-presentation::TC-003
#[test]
fn missing_pose_is_invalid_data_with_the_character_preserved() {
    let source = replace_once(
        PRESENTATION,
        "fever: (offset: 12, scale: 108, frame: Charged),",
        "",
    );
    let error = parse_character_presentations(&source, &roster()).expect_err("missing pose fails");
    assert!(
        matches!(error, DataErrorCause::InvalidData(ref reason) if reason.contains("alpha") && reason.contains("fever"))
    );
}

// component/character-presentation::TC-004
#[test]
fn missing_audio_cue_is_invalid_data_with_the_character_preserved() {
    let source = replace_once(
        PRESENTATION,
        "fever_enter: \"voice.alpha.fever_enter\",",
        "",
    );
    let error = parse_character_presentations(&source, &roster()).expect_err("missing cue fails");
    assert!(
        matches!(error, DataErrorCause::InvalidData(ref reason) if reason.contains("alpha") && reason.contains("fever_enter"))
    );
}

// component/character-presentation::TC-005
#[test]
fn unsupported_schema_is_typed_and_produces_no_catalog() {
    let source = replace_once(PRESENTATION, "schema_version: 1", "schema_version: 255");
    assert_eq!(
        parse_character_presentations(&source, &roster()),
        Err(DataErrorCause::UnsupportedSchema {
            found: 255,
            supported: 1
        })
    );
}

fn failed_resolution() -> DataResolution<CharacterPresentationCatalog> {
    DataResolution::Failed(DataLoadError {
        path: PathBuf::from("data/presentation/characters.ron"),
        category: DataCategory::Other,
        cause: DataErrorCause::Parse("broken".into()),
    })
}

// component/character-presentation::TC-006
#[test]
fn failed_catalog_uses_complete_slot_specific_fallbacks_once() {
    let roster = roster();
    let mut resolver = CharacterPresentationResolver::new(failed_resolution());
    let a = resolver.resolve(&roster.characters[0], 0);
    let b = resolver.resolve(&roster.characters[1], 1);
    let again = resolver.resolve(&roster.characters[0], 0);

    assert!(a.fallback && b.fallback && again.fallback);
    assert_ne!(a.data.primary_color, b.data.primary_color);
    assert_eq!(a.data.poses.len(), POSES.len());
    assert_eq!(a.data.audio.len(), AUDIO_CUES.len());
    assert!(a.data.audio.values().all(|key| key == "silent"));
    assert_eq!(
        resolver.diagnostics().len(),
        2,
        "one fallback diagnostic per character"
    );
}

// component/character-presentation::TC-007
#[test]
fn a_missing_character_falls_back_without_overwriting_loaded_entries() {
    let roster = roster();
    let full = parse_character_presentations(PRESENTATION, &roster).expect("catalog parses");
    let a_before = full
        .get(&CharacterId("alpha".into()))
        .expect("a exists")
        .clone();
    let partial = CharacterPresentationCatalog::from_entries([a_before.clone()]);
    let mut resolver = CharacterPresentationResolver::new(DataResolution::Loaded(partial));

    let a = resolver.resolve(&roster.characters[0], 0);
    let b = resolver.resolve(&roster.characters[1], 1);
    let b_again = resolver.resolve(&roster.characters[1], 1);

    assert!(!a.fallback);
    assert_eq!(a.data, a_before);
    assert!(b.fallback && b_again.fallback);
    assert_eq!(resolver.diagnostics().len(), 1);
}
