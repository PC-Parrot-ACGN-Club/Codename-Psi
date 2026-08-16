//! Rule configuration coverage from `component/rule-configuration.md`.

mod common;

use common::{
    CHARACTER_A_ID, CHARACTER_B_ID, PLAY_A_SRC, PLAY_B_SRC, PROFILE_ID, PROFILE_SRC,
    PUZZLE_BOOK_SRC, ROSTER_SRC, repository_library, repository_spec,
};
use game_core::{
    config::{
        CharacterId, ConfigError, RuleProfileId, ValidatedRuleLibrary, ValidationLayer,
        parse_character_play, parse_fever_puzzle_book, parse_roster, parse_rule_profile,
    },
    match_spec::{AlgorithmVersions, LockedMatchSpec, MatchRequest},
};

fn profile_id() -> RuleProfileId {
    RuleProfileId(PROFILE_ID.into())
}

fn character(id: &str) -> CharacterId {
    CharacterId(id.into())
}

/// Layer and field path of a validation failure, for table-driven assertions.
fn violation(error: &ConfigError) -> (ValidationLayer, String) {
    match error {
        ConfigError::Validation { layer, path, .. } => (*layer, path.clone()),
        other => panic!("expected a located validation failure, got {other:?}"),
    }
}

/// Replaces the first occurrence of `from` with `to`, asserting it was present.
fn patch(source: &str, from: &str, to: &str) -> String {
    assert!(
        source.contains(from),
        "fixture patch target {from:?} is missing; update the test with the asset"
    );
    source.replacen(from, to, 1)
}

fn library_with(
    profile: &str,
    roster: &str,
    plays: &[&str],
    book: &str,
) -> Result<ValidatedRuleLibrary, ConfigError> {
    ValidatedRuleLibrary::new(
        vec![parse_rule_profile(profile)?],
        parse_roster(roster)?,
        plays
            .iter()
            .map(|source| parse_character_play(source))
            .collect::<Result<Vec<_>, _>>()?,
        vec![parse_fever_puzzle_book(book)?],
    )
}

// component/rule-configuration::TC-001
#[test]
fn a_valid_profile_and_content_library_freeze_into_a_two_player_spec() {
    let library = repository_library();
    let request = MatchRequest {
        rule_profile_id: profile_id(),
        root_seed: 0x1,
        characters: [character(CHARACTER_A_ID), character(CHARACTER_B_ID)],
    };
    let spec = LockedMatchSpec::freeze(request.clone(), &library).expect("selection freezes");

    assert_eq!(spec.profile_id, request.rule_profile_id);
    assert_eq!(spec.root_seed, request.root_seed);
    assert_eq!(spec.characters, request.characters);

    // The digest tree travels with the match: the root plus each subject this
    // match actually used.
    assert_eq!(spec.digests.root, library.root_digest());
    assert_eq!(spec.digests.roster, library.roster_digest());
    assert_eq!(
        spec.digests.profile,
        library.profile_digest(&profile_id()).unwrap()
    );
    assert_eq!(
        spec.digests.plays[0],
        library
            .play_digest(&profile_id(), &character(CHARACTER_A_ID))
            .unwrap()
    );
    assert_eq!(spec.algorithms, AlgorithmVersions::current());

    // `MatchRequest` has no opponent-type field to set, so single player,
    // local versus and LAN all freeze the same rule projection: freezing the
    // same request twice cannot diverge.
    let again = LockedMatchSpec::freeze(request, &library).expect("selection freezes again");
    assert_eq!(spec, again);
}

// component/rule-configuration::TC-002
#[test]
fn broken_references_and_out_of_range_coordinates_fail_general_integrity() {
    // Play data pointing at a profile that does not exist.
    let orphan_play = patch(
        PLAY_A_SRC,
        r#"profile_id: ("fever-r1")"#,
        r#"profile_id: ("nope")"#,
    );
    let error = library_with(
        PROFILE_SRC,
        ROSTER_SRC,
        &[&orphan_play, PLAY_B_SRC],
        PUZZLE_BOOK_SRC,
    )
    .expect_err("an unresolvable profile reference is rejected");
    let (layer, path) = violation(&error);
    assert_eq!(layer, ValidationLayer::Integrity);
    assert!(path.contains("profile_id"), "path was {path}");

    // Spawn column outside the declared width.
    let bad_spawn = patch(PROFILE_SRC, "spawn_column: 2", "spawn_column: 6");
    let (layer, path) =
        violation(&parse_rule_profile(&bad_spawn).expect_err("spawn column 6 is off the board"));
    assert_eq!(layer, ValidationLayer::Integrity);
    assert_eq!(path, "profile.field");

    // Puzzle cell outside the declared height.
    let bad_cell = patch(PUZZLE_BOOK_SRC, "(x: 0, y: 13,", "(x: 0, y: 14,");
    let error = library_with(
        PROFILE_SRC,
        ROSTER_SRC,
        &[PLAY_A_SRC, PLAY_B_SRC],
        &bad_cell,
    )
    .expect_err("a puzzle cell off the board is rejected");
    let (layer, path) = violation(&error);
    assert_eq!(layer, ValidationLayer::Integrity);
    assert!(path.contains("cells"), "path was {path}");

    // Two roster entries sharing one id.
    let duplicate = patch(ROSTER_SRC, r#"(id: ("psi-b")"#, r#"(id: ("psi-a")"#);
    let (layer, path) =
        violation(&parse_roster(&duplicate).expect_err("duplicate ids are rejected"));
    assert_eq!(layer, ValidationLayer::Integrity);
    assert!(path.contains("roster.characters"), "path was {path}");

    // The same defect classifies the same way every time.
    let repeat = violation(&parse_roster(&duplicate).expect_err("duplicate ids are rejected"));
    assert_eq!(repeat.0, ValidationLayer::Integrity);
}

// component/rule-configuration::TC-003
#[test]
fn broken_timing_and_drop_set_structure_fail_general_integrity() {
    let cases: [(String, &str); 3] = [
        (
            patch(
                PROFILE_SRC,
                "natural_fall_ticks: 16",
                "natural_fall_ticks: 0",
            ),
            "profile.drop.natural_fall_ticks",
        ),
        (
            patch(PROFILE_SRC, "max_time_ticks: 1800", "max_time_ticks: 300"),
            "profile.fever",
        ),
        (
            patch(PROFILE_SRC, "min_level: 3", "min_level: 16"),
            "profile.fever.min_level",
        ),
    ];
    for (source, expected_path) in cases {
        let (layer, path) =
            violation(&parse_rule_profile(&source).expect_err("integrity defect is rejected"));
        assert_eq!(layer, ValidationLayer::Integrity, "for {expected_path}");
        assert_eq!(path, expected_path);
    }

    // A cycle that is not sixteen hands long.
    let short_cycle = patch(PLAY_A_SRC, "        (shape: I),\n", "");
    let (layer, path) =
        violation(&parse_character_play(&short_cycle).expect_err("15 hands is not a cycle"));
    assert_eq!(layer, ValidationLayer::Integrity);
    assert!(path.ends_with("drop_set"), "path was {path}");

    // A three-ball hand without its color layout.
    let no_layout = patch(
        PLAY_A_SRC,
        "(shape: L, vertical_pair_first: Some(true))",
        "(shape: L)",
    );
    let (layer, path) =
        violation(&parse_character_play(&no_layout).expect_err("an L hand needs a layout"));
    assert_eq!(layer, ValidationLayer::Integrity);
    assert!(path.contains("vertical_pair_first"), "path was {path}");

    // A curve that is not 24 samples long.
    let short_curve = patch(PLAY_A_SRC, "normal: [4, 12,", "normal: [12,");
    let (layer, path) =
        violation(&parse_character_play(&short_curve).expect_err("23 samples is not a curve"));
    assert_eq!(layer, ValidationLayer::Integrity);
    assert!(path.contains("chain_power.normal"), "path was {path}");
}

// component/rule-configuration::TC-004
#[test]
fn incoherent_profile_values_fail_profile_consistency() {
    let cases: [(String, &str); 3] = [
        (
            patch(PROFILE_SRC, "gauge_capacity: 7", "gauge_capacity: 0"),
            "profile.fever.gauge_capacity",
        ),
        (
            patch(PROFILE_SRC, "on_all_clear: 2", "on_all_clear: 13"),
            "profile.fever.level_ladder.on_all_clear",
        ),
        (
            patch(
                PROFILE_SRC,
                "queue_limit: 100000",
                "queue_limit: 4294967295",
            ),
            "profile.nuisance.queue_limit",
        ),
    ];
    for (source, expected_path) in cases {
        let (layer, path) =
            violation(&parse_rule_profile(&source).expect_err("incoherent values are rejected"));
        assert_eq!(
            layer,
            ValidationLayer::ProfileConsistency,
            "for {expected_path}"
        );
        assert_eq!(path, expected_path);
    }
}

// component/rule-configuration::TC-005
#[test]
fn content_that_does_not_cover_the_profile_fails_the_coverage_layer() {
    // A level the puzzle book does not reach.
    let thin_book = patch(
        PUZZLE_BOOK_SRC,
        r#"(id: "lv15-a", level: 15"#,
        r#"(id: "lv15-a", level: 14"#,
    );
    let error = library_with(
        PROFILE_SRC,
        ROSTER_SRC,
        &[PLAY_A_SRC, PLAY_B_SRC],
        &thin_book,
    )
    .expect_err("an uncovered level is rejected");
    let (layer, path) = violation(&error);
    assert_eq!(layer, ValidationLayer::ContentCoverage);
    assert!(path.contains("puzzles"), "path was {path}");

    // A character with no gameplay data only loses that character.
    let (library, report) = ValidatedRuleLibrary::partial(
        vec![parse_rule_profile(PROFILE_SRC).unwrap()],
        parse_roster(ROSTER_SRC).unwrap(),
        vec![parse_character_play(PLAY_A_SRC).unwrap()],
        vec![parse_fever_puzzle_book(PUZZLE_BOOK_SRC).unwrap()],
    )
    .expect("the profile itself is still usable");
    assert_eq!(report.unavailable_characters.len(), 1);
    assert_eq!(
        report.unavailable_characters[0].character,
        character(CHARACTER_B_ID)
    );
    assert!(
        library
            .character_play(&profile_id(), &character(CHARACTER_A_ID))
            .is_some(),
        "the valid character stays selectable"
    );
    assert!(
        LockedMatchSpec::freeze(
            MatchRequest {
                rule_profile_id: profile_id(),
                root_seed: 1,
                characters: [character(CHARACTER_A_ID), character(CHARACTER_B_ID)],
            },
            &library,
        )
        .is_err(),
        "the unavailable character cannot be selected"
    );
}

// component/rule-configuration::TC-006
#[test]
fn digests_ignore_field_order_and_whitespace_but_not_values() {
    let baseline = repository_library().profile_digest(&profile_id()).unwrap();

    // Whitespace and comments are encoding, not content.
    let reflowed = PROFILE_SRC
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let reflowed_digest = library_with(
        &reflowed,
        ROSTER_SRC,
        &[PLAY_A_SRC, PLAY_B_SRC],
        PUZZLE_BOOK_SRC,
    )
    .expect("a reflowed profile is the same profile")
    .profile_digest(&profile_id())
    .unwrap();
    assert_eq!(reflowed_digest, baseline);

    // Reordering two top-level sections keeps the parsed model identical.
    let profile = parse_rule_profile(PROFILE_SRC).unwrap();
    let reordered = parse_rule_profile(&reflowed).unwrap();
    assert_eq!(profile, reordered);

    // Changing one value moves the digest.
    let changed = patch(
        PROFILE_SRC,
        "clear_preview_ticks: 24",
        "clear_preview_ticks: 25",
    );
    let changed_digest = library_with(
        &changed,
        ROSTER_SRC,
        &[PLAY_A_SRC, PLAY_B_SRC],
        PUZZLE_BOOK_SRC,
    )
    .expect("the changed profile is still valid")
    .profile_digest(&profile_id())
    .unwrap();
    assert_ne!(changed_digest, baseline);
}

// component/rule-configuration::TC-007
#[test]
fn editing_one_character_moves_only_that_subject_and_the_root() {
    let before = repository_library();
    // Character B's normal anchor 380 -> 382, with the table regenerated.
    let edited_b = patch(PLAY_B_SRC, "normal_anchor: 380", "normal_anchor: 382");
    let after = library_with(
        PROFILE_SRC,
        ROSTER_SRC,
        &[PLAY_A_SRC, &edited_b],
        PUZZLE_BOOK_SRC,
    )
    .expect("the edited character is still valid");

    let a = character(CHARACTER_A_ID);
    let b = character(CHARACTER_B_ID);
    assert_ne!(
        before.play_digest(&profile_id(), &b),
        after.play_digest(&profile_id(), &b),
        "the edited subject must move"
    );
    assert_ne!(
        before.root_digest(),
        after.root_digest(),
        "the root must move"
    );
    assert_eq!(
        before.play_digest(&profile_id(), &a),
        after.play_digest(&profile_id(), &a),
        "an untouched character subject must not move"
    );
    assert_eq!(before.roster_digest(), after.roster_digest());
    assert_eq!(
        before.profile_digest(&profile_id()),
        after.profile_digest(&profile_id())
    );
}

// component/rule-configuration::TC-008
#[test]
fn a_frozen_spec_does_not_observe_later_asset_changes() {
    let spec = repository_spec(0x1);
    let frozen_preview = spec.resolution.clear_preview_ticks;
    let frozen_digest = spec.digests.profile;

    let changed = patch(
        PROFILE_SRC,
        "clear_preview_ticks: 24",
        "clear_preview_ticks: 20",
    );
    let reloaded = library_with(
        &changed,
        ROSTER_SRC,
        &[PLAY_A_SRC, PLAY_B_SRC],
        PUZZLE_BOOK_SRC,
    )
    .expect("the reloaded profile is valid");
    let respun = LockedMatchSpec::freeze(
        MatchRequest {
            rule_profile_id: profile_id(),
            root_seed: 0x1,
            characters: [character(CHARACTER_A_ID), character(CHARACTER_B_ID)],
        },
        &reloaded,
    )
    .expect("the reloaded selection freezes");

    assert_eq!(spec.resolution.clear_preview_ticks, frozen_preview);
    assert_eq!(spec.digests.profile, frozen_digest);
    assert_eq!(respun.resolution.clear_preview_ticks, 20);
    assert_ne!(respun.digests.profile, frozen_digest);
}

// component/rule-configuration::TC-009
#[test]
fn unusable_rule_data_never_produces_a_locked_spec() {
    // An unknown profile.
    let library = repository_library();
    assert!(
        LockedMatchSpec::freeze(
            MatchRequest {
                rule_profile_id: RuleProfileId("missing".into()),
                root_seed: 0,
                characters: [character(CHARACTER_A_ID), character(CHARACTER_B_ID)],
            },
            &library,
        )
        .is_err(),
        "an unknown profile blocks the match"
    );

    // A character with no gameplay data under the selected profile.
    assert!(
        LockedMatchSpec::freeze(
            MatchRequest {
                rule_profile_id: profile_id(),
                root_seed: 0,
                characters: [character(CHARACTER_A_ID), character("psi-z")],
            },
            &library,
        )
        .is_err(),
        "an unselectable character blocks the match"
    );

    // Data that failed validation never becomes a library in the first place.
    let broken = patch(PROFILE_SRC, "gauge_capacity: 7", "gauge_capacity: 0");
    assert!(
        library_with(
            &broken,
            ROSTER_SRC,
            &[PLAY_A_SRC, PLAY_B_SRC],
            PUZZLE_BOOK_SRC
        )
        .is_err(),
        "invalid data does not reach the freeze entry point"
    );
}

// component/rule-configuration::TC-010
#[test]
fn durations_are_ticks_and_margin_reads_an_integer_table() {
    let spec = repository_spec(1);

    assert_eq!(spec.resolution.clear_preview_ticks, 24);
    assert_eq!(spec.drop.natural_fall_ticks, 16);
    assert_eq!(spec.drop.soft_drop_ticks, 2);
    assert_eq!(spec.drop.lock_delay_ticks, 32);
    assert_eq!(spec.drop.split_delay_pivot_ticks, 1);
    assert_eq!(spec.drop.split_delay_follower_ticks, 2);

    // Target points come from a table index, never a live computation.
    assert_eq!(spec.target_points, 120);
    assert_eq!(spec.margin.target_points_by_step[0], 120);
    assert_eq!(spec.margin.target_points_by_step[1], 90);
    assert_eq!(spec.margin.step_at(0), 0);
    assert_eq!(spec.margin.target_points_at(0), 120);
    assert_eq!(spec.margin.step_at(spec.margin.start_ticks), 1);
    assert_eq!(spec.margin.target_points_at(spec.margin.start_ticks), 90);
    // Past the tail the index saturates rather than wrapping.
    let far = spec.margin.start_ticks + spec.margin.step_ticks * 1_000;
    assert_eq!(
        spec.margin.target_points_at(far),
        *spec.margin.target_points_by_step.last().unwrap()
    );
}
