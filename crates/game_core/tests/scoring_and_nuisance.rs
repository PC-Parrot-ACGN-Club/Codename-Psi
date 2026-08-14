//! Executable component coverage for `component/scoring-and-attack.md`.

mod common;

use common::{A_FEVER, A_NORMAL, B_FEVER, B_NORMAL, TARGET_POINTS, color_bonus, group_bonus};
use game_core::{
    nuisance::{MAX_NUISANCE_DROP, NuisanceDropState, drop_nuisance, offset_attack},
    resolution::ChainLinkFacts,
    rules::{BoardMode, ChainPowerProfile},
    scoring::{AttackFraction, MarginState, ScoreState, ScoringRules},
};

fn profile(normal: [u16; 24], fever: [u16; 24]) -> ChainPowerProfile {
    ChainPowerProfile::new(normal, fever).expect("published table is in domain")
}

fn fever_rules() -> ScoringRules {
    ScoringRules::new(color_bonus(), group_bonus())
}

fn link(index: u8, colors: u8, groups: &[u8]) -> ChainLinkFacts {
    ChainLinkFacts {
        chain_index: index,
        cleared_colored_coords: Vec::new(),
        cleared_nuisance_coords: Vec::new(),
        cleared_colored: groups.iter().map(|&group| u16::from(group)).sum(),
        color_count: colors,
        group_sizes: groups.to_vec(),
    }
}

// component/scoring-and-attack::TC-001
#[test]
fn single_multi_group_and_multi_color_links_follow_the_scoring_formula() {
    let rules = fever_rules();
    let power = profile(A_NORMAL, A_FEVER);
    let score = |link: ChainLinkFacts| rules.score_link(&link, &power, BoardMode::Normal);

    // CP=4, CB=0, GB=0 -> 40 x 4
    assert_eq!(score(link(1, 1, &[4])), 160);
    // CP=4, CB=0, GB=1 -> 50 x 5
    assert_eq!(score(link(1, 1, &[5])), 250);
    // CP=4, CB=2, GB=0 -> 80 x 6
    assert_eq!(score(link(1, 2, &[4, 4])), 480);
    // CP=24, CB=4, GB=0 -> 120 x 28
    assert_eq!(score(link(3, 3, &[4, 4, 4])), 3_360);
}

// component/scoring-and-attack::TC-002
#[test]
fn the_multiplier_sum_is_clamped_at_both_ends_but_the_ball_count_is_not() {
    let rules = fever_rules();

    let floor = profile([1; 24], [1; 24]);
    assert_eq!(
        rules.score_link(&link(1, 1, &[4]), &floor, BoardMode::Normal),
        40,
        "CP=1 CB=0 GB=0 is already the lower clamp"
    );

    // Chain 15 on character A is 999; 999 + 8 + 8 exceeds the ceiling.
    let ceiling = profile(A_NORMAL, A_FEVER);
    assert_eq!(A_NORMAL[14], 999);
    assert_eq!(
        rules.score_link(&link(15, 4, &[4, 4, 4, 11]), &ceiling, BoardMode::Normal),
        229_770,
        "230 x 999, so the clamp applies to the multiplier only"
    );
}

// component/scoring-and-attack::TC-003
#[test]
fn soft_drop_points_are_display_only_and_never_reach_the_conversion() {
    let mut score = ScoreState::default();
    score.add_soft_drop_score(60);
    score.add_chain_score(160);

    assert_eq!(score.displayed(), 220);
    assert_eq!(score.attack_score(), 160);

    let mut fraction = AttackFraction::default();
    assert_eq!(fraction.convert(score.attack_score(), TARGET_POINTS), 1);
    assert_eq!(
        fraction.remainder(),
        40,
        "40 of 120 score units, i.e. the documented 1/3"
    );
}

// component/scoring-and-attack::TC-004
#[test]
fn per_link_conversion_totals_the_same_as_converting_the_whole_chain_once() {
    let mut by_link = AttackFraction::default();
    let per_link: Vec<u64> = [160, 480, 960]
        .into_iter()
        .map(|points| by_link.convert(points, TARGET_POINTS))
        .collect();

    let mut once = AttackFraction::default();
    let whole = once.convert(160 + 480 + 960, TARGET_POINTS);

    assert_eq!(per_link, [1, 4, 8]);
    assert_eq!(per_link.iter().sum::<u64>(), whole);
    assert_eq!((whole, once.remainder()), (13, 40));
    assert_eq!(by_link.remainder(), once.remainder());
}

// component/scoring-and-attack::TC-005
#[test]
fn both_characters_read_both_curves_by_chain_index_and_saturate_at_the_tail() {
    for (normal, fever, expected_normal, expected_fever) in [
        (A_NORMAL, A_FEVER, [4, 440, 999, 999], [4, 248, 840, 840]),
        (B_NORMAL, B_FEVER, [4, 380, 999, 999], [4, 275, 940, 940]),
    ] {
        let curve = profile(normal, fever);
        let read = |mode, steps: [u8; 4]| steps.map(|step| curve.power(mode, step));
        assert_eq!(read(BoardMode::Normal, [1, 10, 24, 25]), expected_normal);
        assert_eq!(read(BoardMode::Fever, [1, 10, 24, 25]), expected_fever);
    }
}

// component/scoring-and-attack::TC-006
#[test]
fn the_same_link_converts_differently_after_swapping_characters() {
    let rules = fever_rules();
    let facts = link(4, 1, &[4]);

    let mut attacks = Vec::new();
    for (normal, fever) in [(A_NORMAL, A_FEVER), (B_NORMAL, B_FEVER)] {
        let score = rules.score_link(&facts, &profile(normal, fever), BoardMode::Normal);
        let mut fraction = AttackFraction::default();
        attacks.push((
            score,
            fraction.convert(score, TARGET_POINTS),
            fraction.remainder(),
        ));
    }

    // A: CP=33 -> 1320 = 11 x 120 exactly. B: CP=29 -> 1160 = 9 x 120 + 80.
    assert_eq!(attacks, [(1_320, 11, 0), (1_160, 9, 80)]);
}

// component/scoring-and-attack::TC-007
#[test]
fn the_remainder_carries_across_drops_and_conserves_the_attack_total() {
    let scores = [1_600_u64, 160, 160];
    let mut fraction = AttackFraction::default();
    let per_drop: Vec<u64> = scores
        .iter()
        .map(|&score| fraction.convert(score, TARGET_POINTS))
        .collect();

    assert_eq!(per_drop, [13, 1, 2]);
    assert_eq!(fraction.remainder(), 0);

    let total: u64 = scores.iter().sum();
    assert_eq!(per_drop.iter().sum::<u64>(), total / TARGET_POINTS);
    assert_eq!(total, 1_920);
}

// component/scoring-and-attack::TC-008
#[test]
fn margin_advances_an_index_and_the_target_score_comes_from_the_table() {
    let table = [120_u64, 90];
    let mut margin = MarginState::default();

    assert_eq!(margin.table_index(), 0);
    assert_eq!(margin.target_points(&table), Some(120));
    let mut before = AttackFraction::default();
    assert_eq!(before.convert(360, 120), 3);

    margin.advance(&table);

    assert_eq!(margin.table_index(), 1);
    assert_eq!(margin.target_points(&table), Some(90));
    let mut after = AttackFraction::default();
    assert_eq!(after.convert(360, 90), 4);
}

// component/scoring-and-attack::TC-009
#[test]
fn offset_consumes_the_active_queue_first_and_never_underflows() {
    let cases = [
        // (active, other, attack) -> (active, other, offset, sent)
        ((3, 4, 2), (1, 4, 2, 0)),
        ((3, 4, 5), (0, 2, 5, 0)),
        ((3, 4, 10), (0, 0, 7, 3)),
        ((0, 0, 6), (0, 0, 0, 6)),
    ];
    for ((queued_active, queued_other, attack), expected) in cases {
        let mut active = queued_active;
        let mut other = queued_other;
        let facts = offset_attack(attack, &mut active, &mut other);
        assert_eq!(
            (active, other, facts.offset, facts.sent),
            expected,
            "attack {attack} against queues {queued_active}/{queued_other}"
        );
    }
}

// component/scoring-and-attack::TC-012
#[test]
fn a_release_is_bounded_by_the_single_drop_cap_and_keeps_the_rest_queued() {
    for (pending, dropped, remaining) in [(29_u32, 29_u32, 0_u32), (30, 30, 0), (35, 30, 5)] {
        let mut queue = pending;
        let mut state = NuisanceDropState::default();
        let drop = drop_nuisance(&mut queue, &mut state);
        assert_eq!((drop.dropped, drop.remaining), (dropped, remaining));
        assert_eq!(drop.columns.len(), dropped as usize);
        assert_eq!(queue, remaining);
    }
    assert_eq!(MAX_NUISANCE_DROP, 30);
}

// component/scoring-and-attack::TC-013
#[test]
fn an_incomplete_row_advances_the_column_order_along_its_two_branches() {
    let mut one = 13;
    let mut one_state = NuisanceDropState::default();
    let first = drop_nuisance(&mut one, &mut one_state);
    assert_eq!(first.columns, [0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5, 0]);
    assert_eq!(
        one_state.next_column(),
        1,
        "one left over starts the next release on the following column"
    );

    let mut two = 14;
    let mut two_state = NuisanceDropState::default();
    let first = drop_nuisance(&mut two, &mut two_state);
    assert_eq!(&first.columns[12..], [0, 1]);
    assert_eq!(
        two_state.next_column(),
        1,
        "two or more left over restart on the column the last ball used"
    );
}
