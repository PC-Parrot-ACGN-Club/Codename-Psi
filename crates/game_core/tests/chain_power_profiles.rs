//! Offline content checks from `component/rule-configuration.md` TC-011–015.

mod common;

use common::{
    A_FEVER, A_NORMAL, B_FEVER, B_NORMAL, CHARACTER_A, CHARACTER_B, TIER_A360_NORMAL,
    TIER_A400_NORMAL, TIER_F36_FEVER, TIER_F40_FEVER,
};
use game_core::rules::{
    BoardMode, ChainPowerParameters, ChainPowerProfile, generate_chain_power_profile,
    verify_chain_power_profile,
};

// component/rule-configuration::TC-011
#[test]
fn the_definition_tiers_regenerate_point_by_point_including_the_saturated_tail() {
    let definition = generate_chain_power_profile(ChainPowerParameters {
        normal_anchor: 400.0,
        normal_tilt: 1.0,
        normal_growth: 0.25,
        fever_anchor: 40.0,
        fever_tilt: 1.0,
    });
    assert_eq!(definition.normal(), &TIER_A400_NORMAL);
    assert_eq!(definition.fever(), &TIER_F40_FEVER);
}

// component/rule-configuration::TC-012
#[test]
fn the_cross_check_tiers_regenerate_point_by_point_including_the_saturated_tail() {
    let cross_check = generate_chain_power_profile(ChainPowerParameters {
        normal_anchor: 360.0,
        normal_tilt: 1.0,
        normal_growth: 0.25,
        fever_anchor: 36.0,
        fever_tilt: 1.0,
    });
    assert_eq!(cross_check.normal(), &TIER_A360_NORMAL);
    assert_eq!(cross_check.fever(), &TIER_F36_FEVER);
    assert_eq!(
        &cross_check.fever()[10..13],
        [252, 259, 308],
        "36 x 7.0, 36 x 7.2 and 36 x 8.55 are the three non-trivial samples"
    );
}

// component/rule-configuration::TC-013
#[test]
fn the_shipped_content_regenerates_from_its_own_stored_parameters() {
    // The design keeps the integer table authoritative and the parameters as
    // provenance. This is the offline check that the two agree, run against
    // the shipped files rather than a transcribed copy of them.
    for (name, source) in [("A", common::PLAY_A_SRC), ("B", common::PLAY_B_SRC)] {
        let play = game_core::config::parse_character_play(source).expect("shipped play parses");
        let stored = play
            .chain_power_profile()
            .expect("shipped curves are in domain");
        let parameters = play.chain_power.source.parameters();
        assert_eq!(
            verify_chain_power_profile(&stored, parameters),
            Ok(()),
            "character {name} tables must regenerate from their stored parameters"
        );
    }
}

// component/rule-configuration::TC-013
#[test]
fn the_shipped_margin_table_regenerates_from_its_own_stored_parameters() {
    let profile =
        game_core::config::parse_rule_profile(common::PROFILE_SRC).expect("shipped profile parses");
    let margin = &profile.scoring.margin;
    let parameters = game_core::rules::MarginParameters {
        initial_target_points: profile.scoring.target_points,
        ratio_numerator: margin.source.ratio_numerator,
        ratio_denominator: margin.source.ratio_denominator,
        max_steps: margin.source.max_steps,
    };
    assert_eq!(
        game_core::rules::verify_margin_table(&margin.target_points_by_step, parameters),
        Ok(())
    );
    assert_eq!(margin.target_points_by_step[0], 120);
    assert_eq!(margin.target_points_by_step[1], 90);
    assert_eq!(
        *margin.target_points_by_step.last().unwrap(),
        1,
        "decay stops once one point buys a nuisance ball"
    );
}

// component/rule-configuration::TC-013
#[test]
fn both_characters_regenerate_their_published_tables_point_by_point() {
    for (name, parameters, normal, fever) in [
        ("A", CHARACTER_A, A_NORMAL, A_FEVER),
        ("B", CHARACTER_B, B_NORMAL, B_FEVER),
    ] {
        let generated = generate_chain_power_profile(parameters);
        assert_eq!(generated.normal(), &normal, "character {name} normal table");
        assert_eq!(generated.fever(), &fever, "character {name} fever table");

        // The stored table is the runtime authority; regeneration must agree.
        let stored = ChainPowerProfile::new(normal, fever).expect("published table is in domain");
        assert_eq!(verify_chain_power_profile(&stored, parameters), Ok(()));
    }
}

// component/rule-configuration::TC-014
#[test]
fn curves_are_24_bounded_samples_with_a_saturating_reader() {
    for (parameters, normal, fever) in [
        (CHARACTER_A, A_NORMAL, A_FEVER),
        (CHARACTER_B, B_NORMAL, B_FEVER),
    ] {
        let profile = ChainPowerProfile::new(normal, fever).expect("published table is in domain");
        assert_eq!(profile.normal().len(), 24);
        assert_eq!(profile.fever().len(), 24);
        assert!(
            profile
                .normal()
                .iter()
                .chain(profile.fever())
                .all(|value| (1..=999).contains(value))
        );
        assert_eq!(profile.power(BoardMode::Normal, 1), normal[0]);
        assert_eq!(profile.power(BoardMode::Normal, 24), normal[23]);
        for beyond in [25, 100, u8::MAX] {
            assert_eq!(profile.power(BoardMode::Normal, beyond), normal[23]);
            assert_eq!(profile.power(BoardMode::Fever, beyond), fever[23]);
        }
        // A profile built straight from content must read the same way.
        assert_eq!(
            generate_chain_power_profile(parameters).power(BoardMode::Fever, 24),
            fever[23]
        );
    }
}

// component/rule-configuration::TC-015
#[test]
fn a_runtime_valid_manual_sample_is_rejected_by_the_offline_consistency_check() {
    let mut normal = A_NORMAL;
    normal[6] += 1;
    let hand_edited =
        ChainPowerProfile::new(normal, A_FEVER).expect("single edit remains runtime-valid");
    let mismatch =
        verify_chain_power_profile(&hand_edited, CHARACTER_A).expect_err("CI detects hand edit");
    assert_eq!(mismatch.mode, BoardMode::Normal);
    assert_eq!(mismatch.index, 7);
    assert_eq!(mismatch.expected, A_NORMAL[6]);
    assert_eq!(mismatch.actual, A_NORMAL[6] + 1);
}
