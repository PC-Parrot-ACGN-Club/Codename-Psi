//! Fever coverage from `component/fever-mode.md`.

mod common;

use game_core::{
    board::{Board, Cell},
    config::FeverPuzzleBook,
    determinism::{MatchRng, StreamName},
    fever::{FeverState, PuzzleBags, load_puzzle, next_target_level, puzzle_by_id},
    match_spec::LockedMatchSpec,
    player::{FEVER_CHANNEL, NORMAL_CHANNEL, PlayerBattleState},
};

fn spec() -> LockedMatchSpec {
    common::repository_spec(0x1)
}

fn rng() -> MatchRng {
    MatchRng::derive(0x1, 0, 0, 0, StreamName::FeverPuzzle)
}

fn gauge_to_full(state: &mut FeverState) {
    for _ in 0..state.capacity() {
        state.begin_safety_point();
        state.record_offset(true);
    }
}

// component/fever-mode::TC-001
#[test]
fn a_full_gauge_enters_fever_and_freezes_the_normal_channel() {
    let spec = spec();
    let mut player = PlayerBattleState::new(&spec, 0, 0, 0);
    // Give the normal channel a board and a queue that must survive the switch.
    let mut board = Board::with_geometry(spec.board_geometry);
    board.set(board.coord(0, 13).expect("in range"), Cell::Color(1));
    player.set_board(board.clone());
    player.set_pending(NORMAL_CHANNEL, 4);

    let capacity = spec.fever.gauge_capacity;
    for cell in 1..capacity {
        player.fever_mut().begin_safety_point();
        player.fever_mut().record_offset(true);
        assert!(!player.fever().is_full(), "cell {cell} does not fill it");
        assert!(!player.fever().active());
        assert_eq!(player.active_channel(), NORMAL_CHANNEL);
    }
    player.fever_mut().begin_safety_point();
    player.fever_mut().record_offset(true);
    assert!(player.fever().is_full());

    assert!(player.enter_fever(&spec, 0), "a full gauge enters");
    assert!(player.fever().active());
    assert_eq!(player.fever().gauge(), 0, "the gauge resets on entry");
    assert_eq!(player.active_channel(), FEVER_CHANNEL);
    assert_eq!(
        player.channel_board(NORMAL_CHANNEL),
        Some(&board),
        "the frozen channel keeps its board"
    );
    assert_eq!(player.pending(NORMAL_CHANNEL), 4, "and its queue");
    assert!(player.session().is_some(), "a session is open");
    assert!(
        !player
            .channel_board(FEVER_CHANNEL)
            .expect("channel exists")
            .visible_is_empty(),
        "the first puzzle is loaded"
    );
}

// component/fever-mode::TC-002
#[test]
fn one_safety_point_adds_at_most_one_gauge_cell() {
    let mut state = FeverState::new(7, 1200, 0, 1800);
    state.begin_safety_point();
    for _ in 0..3 {
        state.record_offset(true);
    }
    assert_eq!(
        state.gauge(),
        1,
        "three offsets in one safety point add one"
    );

    gauge_to_full(&mut state);
    assert_eq!(state.gauge(), state.capacity());
    state.begin_safety_point();
    state.record_offset(true);
    assert_eq!(
        state.gauge(),
        state.capacity(),
        "a full gauge stops growing"
    );
}

// component/fever-mode::TC-003
#[test]
fn time_rewards_clamp_to_the_declared_upper_bound() {
    let mut state = FeverState::new(7, 1750, 0, 1800);
    state.reward_time(300);
    assert_eq!(state.time_ticks(), 1800, "clamped, not 2050");
    assert_eq!(state.time_seconds(), 30, "the display floors to seconds");
    state.reward_time(300);
    assert_eq!(state.time_ticks(), 1800, "already at the bound");
}

// component/fever-mode::TC-004
#[test]
fn an_all_clear_outside_fever_still_adds_player_level_time() {
    for (start, expected) in [(1000_u32, 1300_u32), (1750, 1800)] {
        let mut state = FeverState::new(7, start, 0, 1800);
        assert!(!state.active(), "the player has never entered Fever");
        state.reward_time(300);
        assert_eq!(state.time_ticks(), expected);
        // The value is player level: entering and leaving does not reset it.
        state.enter();
        state.exit();
        assert_eq!(state.time_ticks(), expected);
    }
}

// component/fever-mode::TC-005
#[test]
fn the_offset_time_reward_goes_to_the_attacker_whose_chain_was_offset() {
    let mut attacker = FeverState::new(7, 600, 0, 1800);
    let blocker = FeverState::new(7, 600, 0, 1800);
    // Only the side whose attack was cancelled is rewarded.
    attacker.reward_time(60);
    assert_eq!(attacker.time_ticks(), 660);
    assert_eq!(blocker.time_ticks(), 600, "the blocker gains nothing");
}

// component/fever-mode::TC-006
#[test]
fn a_fever_reward_is_paid_on_the_last_links_preview_tick() {
    let mut state = FeverState::new(7, 600, 0, 1800);
    state.enter();
    state.defer_reward(60);
    assert_eq!(state.time_ticks(), 600, "nothing is paid before the tick");
    assert_eq!(state.deferred_reward(), 60);

    let paid = state.release_deferred_reward();
    assert_eq!(paid, 60);
    assert_eq!(state.time_ticks(), 660);

    // Settlement must not pay it a second time.
    assert_eq!(state.release_deferred_reward(), 0);
    assert_eq!(state.time_ticks(), 660);
}

// component/fever-mode::TC-007
#[test]
fn a_normal_board_all_clear_loads_the_preset_puzzle_and_adds_time() {
    let spec = spec();
    let mut player = PlayerBattleState::new(&spec, 0, 0, 0);
    assert_eq!(player.fever().gauge(), 0);
    let before = player.fever().time_ticks();

    player
        .fever_mut()
        .reward_time(spec.fever.all_clear_reward_ticks);
    player.load_all_clear_puzzle(&spec);

    assert_eq!(
        player.fever().time_ticks(),
        before + spec.fever.all_clear_reward_ticks
    );
    assert_eq!(
        player.active_channel(),
        NORMAL_CHANNEL,
        "an all clear alone does not enter Fever"
    );
    assert_eq!(player.fever().gauge(), 0, "and does not fill the gauge");
    let preset = puzzle_by_id(&spec.fever.puzzles, &spec.fever.all_clear_puzzle_id)
        .expect("the profile names a puzzle the book holds");
    assert_eq!(preset.level, 4, "the preset asks for a four chain");
    assert!(
        !player
            .channel_board(NORMAL_CHANNEL)
            .expect("channel exists")
            .visible_is_empty(),
        "the preset puzzle is on the normal board"
    );
}

// component/fever-mode::TC-008
#[test]
fn all_clear_combinations_set_the_next_target_and_the_time_bonus() {
    let spec = spec();
    let ladder = spec.fever.level_ladder;

    // Inside Fever: meeting the target with an all clear adds two.
    assert_eq!(
        next_target_level(
            ladder,
            10,
            10,
            true,
            spec.fever.min_level,
            spec.fever.max_level
        ),
        12
    );

    // Entering on an all clear starts the first puzzle two levels higher.
    let mut player = PlayerBattleState::new(&spec, 0, 0, 0);
    assert!(player.enter_fever(&spec, ladder.on_all_clear));
    assert_eq!(
        player.session().expect("a session is open").target_level(),
        spec.fever.initial_level + 2
    );

    // The reward is the same in both cases.
    assert_eq!(spec.fever.all_clear_reward_ticks, 300);
}

// component/fever-mode::TC-009
#[test]
fn every_declared_level_can_load_a_legal_puzzle() {
    let spec = spec();
    let book: &FeverPuzzleBook = &spec.fever.puzzles;
    let mut bags = PuzzleBags::new();
    let mut rng = rng();

    for level in spec.fever.min_level..=spec.fever.max_level {
        let id = bags
            .draw(book, level, &mut rng)
            .unwrap_or_else(|| panic!("level {level} must have a puzzle"));
        let puzzle = puzzle_by_id(book, &id).expect("the drawn id is in the book");
        assert_eq!(puzzle.level, level);

        let mut board = Board::with_geometry(spec.board_geometry);
        load_puzzle(&mut board, puzzle);
        for cell in &puzzle.cells {
            let coord = board.coord(cell.x, cell.y).expect("in range");
            assert!(coord.is_visible(), "puzzles never occupy hidden rows");
            assert_eq!(board.get(coord), Cell::Color(cell.color));
            // Nothing floats: the cell below is occupied or is the floor.
            if let Some(below) = board.coord(cell.x, cell.y + 1) {
                assert!(
                    board.get(below).is_occupied(),
                    "puzzle cell ({}, {}) floats",
                    cell.x,
                    cell.y
                );
            }
        }
    }
}

// component/fever-mode::TC-010
#[test]
fn a_level_bag_refills_only_after_it_is_exhausted() {
    let spec = spec();
    let book = &spec.fever.puzzles;
    let level = 10;
    let size = book
        .puzzles
        .iter()
        .filter(|puzzle| puzzle.level == level)
        .count();
    assert!(size >= 1, "the level has content to draw");

    let mut bags = PuzzleBags::new();
    let mut rng = rng();
    let mut drawn = Vec::new();
    for _ in 0..size {
        drawn.push(
            bags.draw(book, level, &mut rng)
                .expect("the bag has content"),
        );
    }
    let mut unique = drawn.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), size, "a bag never repeats before refilling");
    assert!(bags.remaining(level).is_empty(), "the bag is now empty");

    let refilled = bags.draw(book, level, &mut rng).expect("the bag refills");
    assert!(
        book.puzzles
            .iter()
            .any(|puzzle| puzzle.id == refilled && puzzle.level == level),
        "the refill comes from the same level's full set"
    );
}

// component/fever-mode::TC-011
#[test]
fn the_five_puzzle_outcomes_choose_the_next_target_level() {
    let ladder = spec().fever.level_ladder;
    let cases = [
        (10_u8, false, 11_u8),
        (10, true, 12),
        (9, false, 10),
        (8, false, 7),
        (7, false, 5),
    ];
    for (achieved, all_clear, expected) in cases {
        assert_eq!(
            next_target_level(ladder, 10, achieved, all_clear, 3, 15),
            expected,
            "target 10, achieved {achieved}, all clear {all_clear}"
        );
    }
}

// component/fever-mode::TC-012
#[test]
fn ladder_results_clamp_to_both_ends_of_the_level_domain() {
    let ladder = spec().fever.level_ladder;
    assert_eq!(
        next_target_level(ladder, 15, 15, false, 3, 15),
        15,
        "the raw result of 16 clamps down"
    );
    assert_eq!(
        next_target_level(ladder, 3, 0, false, 3, 15),
        3,
        "the raw result of -2 clamps up"
    );

    // A clamped level can still be drawn.
    let spec = spec();
    let mut bags = PuzzleBags::new();
    let mut rng = rng();
    for level in [3_u8, 15] {
        assert!(bags.draw(&spec.fever.puzzles, level, &mut rng).is_some());
    }
}

// component/fever-mode::TC-013
#[test]
fn the_frozen_channel_takes_no_drops_but_can_still_be_offset() {
    let spec = spec();
    let mut player = PlayerBattleState::new(&spec, 0, 0, 0);
    let normal_board = player.board().clone();
    assert!(player.enter_fever(&spec, 0));
    player.set_pending(NORMAL_CHANNEL, 8);
    player.set_pending(FEVER_CHANNEL, 4);

    // A chainless turn releases only onto the active Fever board.
    let landing = player
        .release(&spec, false)
        .expect("a chainless turn releases");
    assert!(landing.dropped > 0);
    assert_eq!(
        player.channel_board(NORMAL_CHANNEL),
        Some(&normal_board),
        "the frozen board takes nothing"
    );
    assert_eq!(player.pending(NORMAL_CHANNEL), 8, "and keeps its queue");

    // Offsetting consumes the active channel first, then the frozen one.
    player.set_pending(FEVER_CHANNEL, 4);
    let facts = player.offset(7);
    assert_eq!(facts.offset, 7);
    assert_eq!(facts.sent, 0);
    assert_eq!(
        player.pending(FEVER_CHANNEL),
        0,
        "the active queue empties first"
    );
    assert_eq!(
        player.pending(NORMAL_CHANNEL),
        5,
        "the frozen queue takes the rest"
    );
}

// component/fever-mode::TC-014
#[test]
fn exiting_fever_merges_the_queues_and_restores_the_normal_channel() {
    let spec = spec();
    let mut player = PlayerBattleState::new(&spec, 0, 0, 0);
    assert!(player.enter_fever(&spec, 0));
    player.set_pending(NORMAL_CHANNEL, 5);
    player.set_pending(FEVER_CHANNEL, 3);
    assert!(player.session().is_some());

    player.exit_fever(&spec);

    assert!(!player.fever().active());
    assert_eq!(player.active_channel(), NORMAL_CHANNEL);
    assert_eq!(
        player.pending(NORMAL_CHANNEL),
        8,
        "the queues merge exactly"
    );
    assert_eq!(player.pending(FEVER_CHANNEL), 0);
    assert!(player.session().is_none(), "the session is discarded");
    assert_eq!(
        player.drop_state(FEVER_CHANNEL).next_column(),
        0,
        "the Fever column order is discarded with it"
    );
    assert!(
        player
            .channel_board(FEVER_CHANNEL)
            .expect("channel exists")
            .visible_is_empty(),
        "the Fever board is discarded"
    );
}

// component/fever-mode::TC-016
#[test]
fn every_settlement_in_fever_switches_the_puzzle_regardless_of_chain_length() {
    let spec = spec();
    let book = &spec.fever.puzzles;
    let mut player = PlayerBattleState::new(&spec, 0, 0, 0);
    assert!(player.enter_fever(&spec, 0));
    assert_eq!(
        player.session().expect("a session is open").target_level(),
        spec.fever.initial_level
    );

    // Alternates chainless locks (achieved = 0) with locks that exactly meet
    // the target, so the level never plateaus between steps. Every step must
    // still switch the puzzle: the design does not gate the switch on chain
    // length (docs/development/design/fever-mode.md §题面循环).
    let steps: [(u8, bool); 4] = [(0, false), (3, false), (4, false), (0, false)];
    let expected_levels = [3_u8, 4, 5, 3];

    for ((achieved, all_clear), expected_level) in steps.into_iter().zip(expected_levels) {
        let before_id = player
            .session()
            .expect("still in Fever")
            .current_puzzle_id()
            .to_string();

        player.advance_fever_puzzle(&spec, achieved, all_clear);

        let session = player.session().expect("still in Fever");
        assert_eq!(
            session.target_level(),
            expected_level,
            "achieved {achieved}, all_clear {all_clear}: the level did not advance, \
             which is exactly what a silently no-op puzzle switch looks like"
        );

        let puzzle =
            puzzle_by_id(book, session.current_puzzle_id()).expect("the drawn id is in the book");
        assert_eq!(puzzle.level, expected_level);

        let mut expected_board = Board::with_geometry(spec.board_geometry);
        load_puzzle(&mut expected_board, puzzle);
        assert_eq!(
            player.channel_board(FEVER_CHANNEL),
            Some(&expected_board),
            "achieved {achieved}: the Fever board must show the new puzzle, not the old one"
        );
        assert_ne!(
            session.current_puzzle_id(),
            before_id,
            "achieved {achieved}: a level change with one puzzle per level must draw a new id"
        );
    }
}

// component/fever-mode::TC-015
#[test]
fn the_drop_right_after_flipping_back_still_obeys_the_batch_limit() {
    let spec = spec();
    let mut player = PlayerBattleState::new(&spec, 0, 0, 0);
    assert!(player.enter_fever(&spec, 0));
    player.set_pending(NORMAL_CHANNEL, 32);
    player.set_pending(FEVER_CHANNEL, 3);
    player.exit_fever(&spec);
    assert_eq!(player.pending(NORMAL_CHANNEL), 35);

    let landing = player
        .release(&spec, false)
        .expect("flipping back triggers one release");
    assert_eq!(landing.dropped, spec.nuisance.drop_limit);
    assert_eq!(landing.dropped, 30);
    assert_eq!(
        player.pending(NORMAL_CHANNEL),
        5,
        "the remainder waits for a later drop"
    );
}
