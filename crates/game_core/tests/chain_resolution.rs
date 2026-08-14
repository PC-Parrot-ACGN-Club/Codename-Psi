//! Executable component coverage for `component/chain-resolution.md`.

use game_core::{
    board::{Board, Cell, Coord},
    resolution::{ResolutionPhase, ResolutionRules, ResolutionState},
};

fn at(x: u8, y: u8) -> Coord {
    Coord::new(x, y).expect("fixture coordinate is valid")
}

fn board(cells: &[(u8, u8, Cell)]) -> Board {
    let mut board = Board::empty();
    for &(x, y, cell) in cells {
        board.set(at(x, y), cell);
    }
    board
}

fn settle(cells: &[(u8, u8, Cell)]) -> ResolutionState {
    let mut state = ResolutionState::new(board(cells), ResolutionRules::default());
    state.settle();
    state
}

/// Stacks `cell` over an inclusive row range so a fixture stands on support
/// instead of floating.
fn column(x: u8, rows: std::ops::RangeInclusive<u8>, cell: Cell) -> Vec<(u8, u8, Cell)> {
    rows.map(|y| (x, y, cell)).collect()
}

// component/chain-resolution::TC-001
#[test]
fn only_visible_groups_at_or_above_the_threshold_clear() {
    let three = settle(&[
        (0, 13, Cell::Color(1)),
        (1, 13, Cell::Color(1)),
        (2, 13, Cell::Color(1)),
    ]);
    let four = settle(&[
        (0, 13, Cell::Color(1)),
        (1, 13, Cell::Color(1)),
        (2, 13, Cell::Color(1)),
        (3, 13, Cell::Color(1)),
    ]);
    let five = settle(&[
        (0, 13, Cell::Color(1)),
        (1, 13, Cell::Color(1)),
        (2, 13, Cell::Color(1)),
        (0, 12, Cell::Color(1)),
        (0, 11, Cell::Color(1)),
    ]);

    assert!(three.report().expect("settled").links.is_empty());
    assert_eq!(four.report().expect("settled").links[0].group_sizes, [4]);
    assert_eq!(five.report().expect("settled").links[0].group_sizes, [5]);
}

// component/chain-resolution::TC-002, TC-003
#[test]
fn simultaneous_groups_are_one_link_with_stable_color_and_group_facts() {
    let state = settle(&[
        (0, 13, Cell::Color(1)),
        (1, 13, Cell::Color(1)),
        (2, 13, Cell::Color(1)),
        (3, 13, Cell::Color(1)),
        (0, 11, Cell::Color(2)),
        (1, 11, Cell::Color(2)),
        (2, 11, Cell::Color(2)),
        (3, 11, Cell::Color(2)),
        (0, 9, Cell::Color(3)),
        (1, 9, Cell::Color(3)),
        (2, 9, Cell::Color(3)),
        (3, 9, Cell::Color(3)),
    ]);
    let link = &state.report().expect("settled").links[0];
    assert_eq!(link.group_sizes, [4, 4, 4]);
    assert_eq!(link.color_count, 3);
    assert_eq!(link.cleared_colored, 12);
}

// component/chain-resolution::TC-004
#[test]
fn adjacent_nuisance_is_deduplicated_and_non_adjacent_nuisance_remains() {
    let state = settle(&[
        (0, 13, Cell::Color(1)),
        (1, 13, Cell::Color(1)),
        (2, 13, Cell::Color(1)),
        (3, 13, Cell::Color(1)),
        (0, 12, Cell::Nuisance),
        (1, 12, Cell::Nuisance),
        (3, 12, Cell::Nuisance),
        (5, 13, Cell::Nuisance),
    ]);
    let report = state.report().expect("settled");
    assert_eq!(report.links[0].cleared_nuisance_coords.len(), 3);
    assert_eq!(state.board().get(at(5, 13)), Cell::Nuisance);
}

// component/chain-resolution::TC-005
#[test]
fn hidden_rows_join_no_group_and_are_out_of_reach_of_adjacent_nuisance_clearing() {
    // Three visible balls plus one directly above them in a hidden row. The
    // hidden ball would complete the group if it counted.
    let mut not_a_group = vec![
        (0, 1, Cell::Color(1)),
        (0, 2, Cell::Color(1)),
        (0, 3, Cell::Color(1)),
        (0, 4, Cell::Color(1)),
    ];
    not_a_group.extend(column(0, 5..=13, Cell::Nuisance));
    let state = settle(&not_a_group);
    assert!(state.report().expect("settled").links.is_empty());
    assert_eq!(state.board().get(at(0, 1)), Cell::Color(1));

    // A clearing group at the top visible row, with nuisance both below it and
    // directly above it in a hidden row.
    let mut with_nuisance = vec![
        (0, 1, Cell::Nuisance),
        (0, 2, Cell::Color(1)),
        (1, 2, Cell::Color(1)),
        (2, 2, Cell::Color(1)),
        (3, 2, Cell::Color(1)),
    ];
    for x in 0..4 {
        with_nuisance.extend(column(x, 3..=13, Cell::Nuisance));
    }
    let state = settle(&with_nuisance);
    let link = &state.report().expect("settled").links[0];
    assert_eq!(link.cleared_nuisance_coords.len(), 4);
    assert!(
        link.cleared_nuisance_coords
            .iter()
            .all(|coord| coord.is_visible()),
        "adjacent-nuisance clearing never reaches a hidden row"
    );
}

// component/chain-resolution::TC-006
#[test]
fn a_hidden_row_ball_falls_into_the_visible_region_and_joins_a_later_link() {
    // Column 0 from the bottom: nine same-colored balls clear as link 1; the
    // four above them include one parked in a hidden row.
    let mut cells = column(0, 5..=13, Cell::Color(2));
    cells.extend(column(0, 1..=4, Cell::Color(1)));
    let state = settle(&cells);

    let report = state.report().expect("settled");
    assert_eq!(report.links.len(), 2, "the hidden ball completes link 2");
    assert_eq!(report.links[0].group_sizes, [9]);
    assert_eq!(
        report.links[1].group_sizes,
        [4],
        "three visible balls only reach the threshold once the hidden one lands"
    );
    assert_eq!(report.total_cleared_colored, 13);
    assert!(report.field.all_clear);
}

// component/chain-resolution::TC-007, TC-012
#[test]
fn gravity_can_form_a_second_link_and_reports_an_all_clear() {
    let state = settle(&[
        (0, 13, Cell::Color(2)),
        (1, 13, Cell::Color(2)),
        (2, 13, Cell::Color(2)),
        (3, 13, Cell::Color(2)),
        (3, 12, Cell::Nuisance),
        (0, 12, Cell::Color(1)),
        (1, 12, Cell::Color(1)),
        (2, 12, Cell::Color(1)),
        (3, 11, Cell::Color(1)),
    ]);
    let report = state.report().expect("settled");
    assert_eq!(report.links.len(), 2);
    assert_eq!(report.links[0].cleared_colored, 4);
    assert_eq!(report.links[0].cleared_nuisance_coords.len(), 1);
    assert_eq!(report.links[1].group_sizes, [4]);
    assert_eq!(report.total_cleared_colored, 8);
    assert!(report.field.all_clear);
}

// component/chain-resolution::TC-008, TC-009, TC-010
#[test]
fn clear_and_gravity_commit_only_on_their_tick_boundaries() {
    let rules = ResolutionRules {
        clear_preview_ticks: 2,
        gravity_ticks_by_distance: vec![0, 3, 3],
        clear_threshold: 4,
    };
    let mut state = ResolutionState::new(
        board(&[
            (0, 13, Cell::Color(2)),
            (1, 13, Cell::Color(2)),
            (2, 13, Cell::Color(2)),
            (3, 13, Cell::Color(2)),
            (0, 12, Cell::Color(1)),
        ]),
        rules,
    );
    assert!(matches!(
        state.phase(),
        ResolutionPhase::ClearPreview {
            elapsed_ticks: 0,
            ..
        }
    ));
    state.tick();
    assert_eq!(state.board().get(at(0, 13)), Cell::Color(2));
    state.tick();
    assert_eq!(
        state.board().get(at(0, 13)),
        Cell::Empty,
        "clear commits at preview expiry"
    );
    assert!(
        matches!(state.phase(), ResolutionPhase::ClearCommit { facts } if facts.chain_index == 1),
        "the tick rests on the zero-tick ClearCommit boundary"
    );
    state.tick();
    assert!(
        matches!(
            state.phase(),
            ResolutionPhase::Gravity {
                elapsed_ticks: 1,
                duration_ticks: 3,
                ..
            }
        ),
        "leaving the boundary and timing gravity share one tick"
    );
    assert_eq!(
        state.board().get(at(0, 12)),
        Cell::Color(1),
        "gravity is still uncommitted"
    );
    state.tick();
    state.tick();
    assert_eq!(
        state.board().get(at(0, 13)),
        Cell::Color(1),
        "gravity commits atomically at expiry"
    );
}

// component/chain-resolution::TC-011
#[test]
fn no_chain_settles_immediately_without_entering_a_preview() {
    let board = board(&[
        (0, 13, Cell::Color(1)),
        (1, 13, Cell::Color(1)),
        (2, 13, Cell::Color(1)),
    ]);
    let state = ResolutionState::new(board, ResolutionRules::default());
    assert!(
        matches!(state.phase(), ResolutionPhase::Settlement(report) if report.links.is_empty())
    );
    assert!(!state.report().expect("settled").field.all_clear);
}

// component/chain-resolution::TC-016
#[test]
fn an_idle_resolution_only_leaves_that_phase_when_a_lock_triggers_it() {
    let cells = [
        (0, 13, Cell::Color(1)),
        (1, 13, Cell::Color(1)),
        (2, 13, Cell::Color(1)),
        (3, 13, Cell::Color(1)),
    ];
    let mut state = ResolutionState::idle(board(&cells), ResolutionRules::default());

    // Ticks alone never start a resolution, and the board is left untouched.
    for _ in 0..30 {
        state.tick();
    }
    assert_eq!(state.phase(), &ResolutionPhase::Idle);
    assert_eq!(state.board().get(at(0, 13)), Cell::Color(1));
    assert!(state.report().is_none());

    state.lock();
    assert!(
        matches!(
            state.phase(),
            ResolutionPhase::ClearPreview {
                elapsed_ticks: 0,
                ..
            }
        ),
        "the lock scans and opens the first preview"
    );

    // A second lock during a running resolution changes nothing.
    let running = state.phase().clone();
    state.lock();
    assert_eq!(state.phase(), &running);

    let report = state.settle().clone();
    assert_eq!(report.links.len(), 1);
    // `new` is the same thing with the lock already applied.
    let mut locked = ResolutionState::new(board(&cells), ResolutionRules::default());
    assert_eq!(locked.settle(), &report);
}

// component/chain-resolution::TC-015
#[test]
fn neither_zero_tick_boundary_lengthens_a_chain_step() {
    let rules = ResolutionRules::default();

    // DEC-004 derives the preview duration from a chain step of roughly 22 to
    // 31 ticks. That range is exactly preview plus a one- to three-cell
    // gravity, which leaves no tick for either boundary.
    assert_eq!(rules.clear_preview_ticks, 12);
    assert_eq!(
        rules.clear_preview_ticks + rules.gravity_ticks_by_distance[1],
        22
    );
    assert_eq!(
        rules.clear_preview_ticks + rules.gravity_ticks_by_distance[3],
        31
    );

    // The two-link fixture from TC-007 falls two cells, so its first step must
    // span preview plus the two-cell gravity and nothing more.
    let mut state = ResolutionState::new(
        board(&[
            (0, 13, Cell::Color(2)),
            (1, 13, Cell::Color(2)),
            (2, 13, Cell::Color(2)),
            (3, 13, Cell::Color(2)),
            (3, 12, Cell::Nuisance),
            (0, 12, Cell::Color(1)),
            (1, 12, Cell::Color(1)),
            (2, 12, Cell::Color(1)),
            (3, 11, Cell::Color(1)),
        ]),
        rules.clone(),
    );
    let step = rules.clear_preview_ticks + rules.gravity_ticks_by_distance[2];
    assert_eq!(step, 27);

    let mut labels = Vec::new();
    for _ in 0..=step {
        labels.push(observed(&state).0);
        state.tick();
    }

    assert_eq!(labels[0], "preview", "link 1 preview starts at tick 0");
    assert_eq!(
        labels[usize::from(rules.clear_preview_ticks)],
        "clear-commit",
        "the preview ends on the ClearCommit boundary"
    );
    assert_eq!(
        labels[usize::from(step)],
        "scan-next",
        "gravity ends on the ScanNext boundary, {step} ticks into the step"
    );
    assert_eq!(
        observed(&state),
        ("preview", 1, rules.clear_preview_ticks),
        "the tick after the boundary already times link 2's preview"
    );
}

/// A phase label plus its progress, i.e. everything presentation may observe.
fn observed(state: &ResolutionState) -> (&'static str, u16, u16) {
    match state.phase() {
        ResolutionPhase::Idle => ("idle", 0, 0),
        ResolutionPhase::ClearPreview {
            elapsed_ticks,
            duration_ticks,
            ..
        } => ("preview", *elapsed_ticks, *duration_ticks),
        ResolutionPhase::ClearCommit { facts } => ("clear-commit", u16::from(facts.chain_index), 0),
        ResolutionPhase::Gravity {
            elapsed_ticks,
            duration_ticks,
            ..
        } => ("gravity", *elapsed_ticks, *duration_ticks),
        ResolutionPhase::ScanNext { next_chain_index } => {
            ("scan-next", u16::from(*next_chain_index), 0)
        }
        ResolutionPhase::Settlement(_) => ("settlement", 0, 0),
    }
}

// component/chain-resolution::TC-014
#[test]
fn neither_the_stepping_pattern_nor_a_presentation_consumer_shifts_the_phase_ticks() {
    // The two-link fixture from TC-007, so both phases run more than once.
    let cells = [
        (0, 13, Cell::Color(2)),
        (1, 13, Cell::Color(2)),
        (2, 13, Cell::Color(2)),
        (3, 13, Cell::Color(2)),
        (3, 12, Cell::Nuisance),
        (0, 12, Cell::Color(1)),
        (1, 12, Cell::Color(1)),
        (2, 12, Cell::Color(1)),
        (3, 11, Cell::Color(1)),
    ];
    let rules = ResolutionRules::default();
    const TICKS: usize = 60;

    // One tick at a time, recording the sequence presentation would see.
    let mut single = ResolutionState::new(board(&cells), rules.clone());
    let mut baseline = Vec::with_capacity(TICKS);
    for _ in 0..TICKS {
        baseline.push(observed(&single));
        single.tick();
    }

    // The same total, advanced in three segments.
    let mut segmented = ResolutionState::new(board(&cells), rules.clone());
    let mut segmented_sequence = Vec::with_capacity(TICKS);
    for segment in [20, 20, 20] {
        for _ in 0..segment {
            segmented_sequence.push(observed(&segmented));
            segmented.tick();
        }
    }

    // A consumer that reads every presentation-facing field on every tick.
    let mut watched = ResolutionState::new(board(&cells), rules);
    let mut watched_sequence = Vec::with_capacity(TICKS);
    for _ in 0..TICKS {
        let _ = watched.board();
        let _ = watched.report();
        watched_sequence.push(observed(&watched));
        let _ = watched.phase();
        watched.tick();
    }

    assert_eq!(segmented_sequence, baseline);
    assert_eq!(watched_sequence, baseline);
    assert_eq!(single.report(), watched.report());
    assert_eq!(single.board(), watched.board());
    assert!(
        baseline.iter().any(|entry| entry.0 == "preview")
            && baseline.iter().any(|entry| entry.0 == "gravity")
            && baseline.last().expect("60 ticks recorded").0 == "settlement",
        "the fixture must actually traverse preview, gravity and settlement"
    );
}
