//! Executable component coverage for `component/scoring-and-attack.md`.

mod common;

use common::{A_FEVER, A_NORMAL, B_FEVER, B_NORMAL, TARGET_POINTS, color_bonus, group_bonus};
use game_core::{
    board::{Board, Cell},
    nuisance::{
        MAX_NUISANCE_DROP, NuisanceDropState, NuisanceRules, drop_nuisance,
        drop_nuisance_with_rules, enqueue, offset_attack, release_nuisance,
    },
    resolution::ChainLinkFacts,
    rules::{BoardMode, ChainPowerProfile},
    safety_point::arbitrate_attacks_with_limit,
    scoring::{AttackFraction, MarginState, ScoreState, ScoringRules},
};

fn profile(normal: [u16; 24], fever: [u16; 24]) -> ChainPowerProfile {
    ChainPowerProfile::new(normal, fever).expect("published table is in domain")
}

#[test]
fn profile_nuisance_rules_control_the_batch_limit_without_changing_column_order() {
    let mut pending = 9;
    let mut state = NuisanceDropState::at_column(0);
    let drop = drop_nuisance_with_rules(
        &mut pending,
        &mut state,
        NuisanceRules {
            drop_limit: 4,
            ..NuisanceRules::default()
        },
    );
    assert_eq!(drop.dropped, 4);
    assert_eq!(drop.columns, vec![0, 1, 2, 3]);
    assert_eq!(drop.remaining, 5);
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
    let spec = common::repository_spec(1);
    let rules = &spec.margin;
    let mut margin = MarginState::default();

    // Before the margin start tick nothing has decayed.
    margin.advance_to(rules, rules.start_ticks - 1);
    assert_eq!(margin.table_index(), 0);
    let target = margin
        .target_points(&rules.target_points_by_step)
        .expect("index 0 is in the table");
    assert_eq!(target, 120);
    let mut before = AttackFraction::default();
    assert_eq!(before.convert(360, target), 3);
    assert_eq!(before.remainder(), 0);

    // The round tick, not a call count, decides the step.
    margin.advance_to(rules, rules.start_ticks);
    assert_eq!(margin.table_index(), 1);
    let target = margin
        .target_points(&rules.target_points_by_step)
        .expect("index 1 is in the table");
    assert_eq!(target, 90);
    let mut after = AttackFraction::default();
    assert_eq!(after.convert(360, target), 4);
    assert_eq!(after.remainder(), 0);

    // The state keeps only the index; nothing caches a converted target.
    assert_eq!(
        std::mem::size_of::<MarginState>(),
        std::mem::size_of::<usize>()
    );
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

// component/scoring-and-attack::TC-010
#[test]
fn a_turn_that_triggered_a_chain_leaves_its_queue_untouched() {
    let mut board = Board::empty();
    let mut pending = 8_u32;
    let mut other = 0_u32;
    let mut state = NuisanceDropState::at_column(0);

    // The turn's attack offsets first; three of the eight are cancelled.
    let facts = offset_attack(3, &mut pending, &mut other);
    assert_eq!(facts.offset, 3);
    assert_eq!(facts.sent, 0);
    assert_eq!(pending, 5);

    let released = release_nuisance(
        &mut board,
        &mut pending,
        &mut state,
        NuisanceRules::default(),
        true,
    );

    assert!(released.is_none(), "a chain suppresses this turn's release");
    assert_eq!(pending, 5, "the rest of the queue waits for a later turn");
    assert_eq!(state.next_column(), 0, "the column order does not advance");
    assert!(
        board
            .visible_coords()
            .all(|coord| board.get(coord) == Cell::Empty),
        "nothing lands on the board"
    );
}

// component/scoring-and-attack::TC-011
#[test]
fn a_turn_without_a_chain_drops_its_queue_into_the_active_board() {
    let mut board = Board::empty();
    let mut pending = 6_u32;
    let mut state = NuisanceDropState::at_column(0);

    let landing = release_nuisance(
        &mut board,
        &mut pending,
        &mut state,
        NuisanceRules::default(),
        false,
    )
    .expect("a chainless turn releases");

    assert_eq!(landing.dropped, 6);
    assert_eq!(landing.remaining, 0);
    assert_eq!(pending, 0);
    let columns: Vec<u8> = landing.coords.iter().map(|coord| coord.x()).collect();
    assert_eq!(
        columns,
        vec![0, 1, 2, 3, 4, 5],
        "a full row fills left to right"
    );
    let rows: Vec<u8> = landing.coords.iter().map(|coord| coord.y()).collect();
    assert!(
        rows.iter().all(|y| *y == rows[0]),
        "six balls on an empty board share one row"
    );
    assert!(
        landing
            .coords
            .iter()
            .all(|coord| board.get(*coord) == Cell::Nuisance),
        "the board holds the landed nuisance and can now run gravity"
    );
}

// component/scoring-and-attack::TC-011
#[test]
fn a_queue_never_grows_past_the_profile_limit() {
    let mut pending = 90_u32;
    let discarded = enqueue(&mut pending, 20, 100);
    assert_eq!(pending, 100, "the limit clamps the queue");
    assert_eq!(discarded, 10, "the overflow is reported, not hidden");

    let report = arbitrate_attacks_with_limit([50, 0], [0, 80], 100);
    assert_eq!(report.queues_after[1], 100);
}
