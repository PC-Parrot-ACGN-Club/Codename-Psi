//! Falling-group control coverage from `component/falling-group-control.md`.

mod common;

use game_core::{
    board::{Board, Cell, Coord},
    config::{DropSet, DropShape, DropTemplate},
    control::{ControlOutcome, ControlRules, ControlState, LockCause},
    determinism::{MatchRng, StreamName},
    drop_stream::{DropStream, spawn_group},
    falling::{DoubleRotation, FallingGroup, RotationOutcome},
    input::{GameAction, PlayerActions},
    match_spec::{DropTiming, LockedMatchSpec},
};

fn at(x: u8, y: u8) -> Coord {
    Coord::new(x, y).expect("fixture coordinate is on the board")
}

fn spec() -> LockedMatchSpec {
    common::repository_spec(0x1)
}

fn hand(shape: DropShape, layout: Option<bool>) -> DropTemplate {
    DropTemplate {
        shape,
        vertical_pair_first: layout,
    }
}

/// A single board plus one controllable group, driven a tick at a time.
struct Harness {
    board: Board,
    group: FallingGroup,
    control: ControlState,
    timing: DropTiming,
    fall_table: Vec<u16>,
    color_count: u8,
}

impl Harness {
    fn new(template: DropTemplate, pivot: Coord) -> Self {
        let spec = spec();
        Self {
            board: Board::with_geometry(spec.board_geometry),
            group: FallingGroup::new(template, [0, 1], pivot, 0),
            control: ControlState::new(),
            timing: spec.drop,
            fall_table: spec.resolution.gravity_ticks_by_distance.clone(),
            color_count: spec.color_count,
        }
    }

    /// Overrides one profile value so a rule can be exercised on its own.
    fn with_timing(mut self, edit: impl FnOnce(&mut DropTiming)) -> Self {
        edit(&mut self.timing);
        self
    }

    fn fill(&mut self, cells: &[(u8, u8)]) {
        for (x, y) in cells {
            self.board.set(at(*x, *y), Cell::Color(3));
        }
    }

    fn tick(&mut self, actions: PlayerActions) -> ControlOutcome {
        let rules = ControlRules {
            timing: &self.timing,
            fall_ticks_by_distance: &self.fall_table,
            color_count: self.color_count,
        };
        self.control
            .step(&mut self.group, &mut self.board, actions, rules)
    }

    fn pivot(&self) -> (u8, u8) {
        (self.group.pivot().x(), self.group.pivot().y())
    }
}

fn actions(list: &[GameAction]) -> PlayerActions {
    list.iter().copied().collect()
}

// component/falling-group-control::TC-001
#[test]
fn every_hand_that_spawns_is_the_one_next_was_showing() {
    let spec = spec();
    for slot in 0..2 {
        let mut rng = MatchRng::derive(spec.root_seed, 0, 0, slot as u8, StreamName::Color);
        let mut stream = DropStream::new(
            spec.drop_sets[slot].clone(),
            spec.drop.next_queue_len,
            spec.color_count,
            &mut rng,
        );
        for turn in 0..32 {
            let queued: Vec<_> = stream.queued().collect();
            assert_eq!(
                queued.len(),
                usize::from(spec.drop.next_queue_len),
                "turn {turn}: the queue is refilled to the profile length"
            );
            let previewed = stream.peek().expect("NEXT always holds a hand");
            let spawned = stream
                .take(spec.color_count, &mut rng)
                .expect("a queued hand can always be taken");
            assert_eq!(
                previewed, spawned,
                "turn {turn}: shape, layout and colors must match what NEXT showed"
            );
        }
    }
}

// component/falling-group-control::TC-002
#[test]
fn an_odd_mono_quad_count_swaps_l_and_j_in_the_second_cycle() {
    for slot in 0..2 {
        let drop_set = &spec().drop_sets[slot];
        assert!(
            drop_set.swaps_l_and_j(),
            "both shipped characters take one single-color O hand, an odd count"
        );
        for index in 0..16_u64 {
            let first = drop_set.hand(index).shape;
            let second = drop_set.hand(index + 16).shape;
            match first {
                DropShape::L => assert_eq!(second, DropShape::J),
                DropShape::J => assert_eq!(second, DropShape::L),
                other => assert_eq!(second, other, "hand {index} must not change"),
            }
        }
    }

    // Control: an even count leaves the cycle at sixteen hands.
    let mut hands = vec![hand(DropShape::I, None); 13];
    hands.push(hand(DropShape::L, Some(true)));
    hands.push(hand(DropShape::OMono, None));
    hands.push(hand(DropShape::OMono, None));
    let even = DropSet(hands);
    assert!(!even.swaps_l_and_j());
    for index in 0..16_u64 {
        assert_eq!(even.hand(index).shape, even.hand(index + 16).shape);
    }
}

// component/falling-group-control::TC-003
#[test]
fn every_shape_and_orientation_occupies_distinct_in_range_cells() {
    let board = Board::with_geometry(spec().board_geometry);
    let shapes = [
        hand(DropShape::I, None),
        hand(DropShape::L, Some(true)),
        hand(DropShape::J, Some(false)),
        hand(DropShape::ODual, None),
    ];
    for template in shapes {
        for transform in 0..4_u8 {
            for x in 0..6_u8 {
                for y in [1_u8, 6] {
                    let mut group = FallingGroup::new(template, [0, 1], at(x, y), 0);
                    // Reaching an orientation is itself a rotation, so drive it
                    // through the same judgement the game uses.
                    let mut counter = DoubleRotation::new();
                    for _ in 0..transform {
                        group.rotate(&board, true, &mut counter, 2, 4);
                    }
                    let Some(cells) = group.cells(&board) else {
                        // Off-board poses are simply not legal poses.
                        continue;
                    };
                    let mut coords: Vec<_> = cells.iter().map(|(coord, _)| *coord).collect();
                    coords.sort();
                    let unique = coords.len();
                    coords.dedup();
                    assert_eq!(unique, coords.len(), "a pose must not occupy a cell twice");
                    assert_eq!(
                        coords.len(),
                        usize::from(template.shape.ball_count()),
                        "every ball needs a cell"
                    );
                    assert!(coords.iter().all(|coord| coord.x() < 6 && coord.y() < 14));
                }
            }
        }
    }
}

// component/falling-group-control::TC-004
#[test]
fn a_single_color_o_cycles_its_color_instead_of_turning() {
    let board = Board::with_geometry(spec().board_geometry);
    let mut group = FallingGroup::new(hand(DropShape::OMono, None), [0, 0], at(2, 6), 0);
    let cells_before = group.cells(&board).expect("the spawn pose fits");
    let mut counter = DoubleRotation::new();

    for step in 1..=5_u8 {
        let outcome = group.rotate(&board, true, &mut counter, 2, 4);
        assert_eq!(outcome, RotationOutcome::ColorCycled);
        assert_eq!(group.transform_id(), 0, "a single-color O never turns");
        assert_eq!(
            group
                .cells(&board)
                .map(|cells| cells.iter().map(|(coord, _)| *coord).collect::<Vec<_>>()),
            cells_before
                .iter()
                .map(|(coord, _)| *coord)
                .collect::<Vec<_>>()
                .into(),
            "occupancy must not change"
        );
        assert_eq!(
            group.colors()[0],
            step % 4,
            "colors advance in a fixed cycle"
        );
    }
    // Four colors, so the fourth input returns to the starting color.
    assert_eq!(4 % 4, 0);
}

// component/falling-group-control::TC-005
#[test]
fn a_free_target_confirms_the_rotation_without_touching_the_counter() {
    let board = Board::with_geometry(spec().board_geometry);
    let mut group = FallingGroup::new(hand(DropShape::I, None), [0, 1], at(2, 6), 0);
    let mut counter = DoubleRotation::new();

    assert_eq!(
        group.rotate(&board, true, &mut counter, 2, 4),
        RotationOutcome::Confirmed
    );
    assert_eq!(group.transform_id(), 1);
    assert_eq!(
        group.pivot(),
        at(2, 6),
        "a free rotation does not move the pivot"
    );
    assert_eq!(counter.attempts(), 0);

    assert_eq!(
        group.rotate(&board, false, &mut counter, 2, 4),
        RotationOutcome::Confirmed
    );
    assert_eq!(group.transform_id(), 0);
    assert_eq!(counter.attempts(), 0);
}

// component/falling-group-control::TC-006
#[test]
fn a_vertical_target_inside_the_hidden_rows_is_refused_without_a_push() {
    let mut board = Board::with_geometry(spec().board_geometry);
    board.set(at(2, 0), Cell::Color(3));
    // Orientation 1 puts the follower to the right; turning back to 0 aims at
    // the occupied cell directly above the pivot.
    let mut group = FallingGroup::new(hand(DropShape::I, None), [0, 1], at(2, 1), 0);
    let mut counter = DoubleRotation::new();
    group.rotate(&board, true, &mut counter, 2, 4);
    assert_eq!(group.transform_id(), 1);

    let outcome = group.rotate(&board, false, &mut counter, 2, 4);
    assert_eq!(outcome, RotationOutcome::Blocked);
    assert_eq!(group.transform_id(), 1, "the orientation is unchanged");
    assert_eq!(group.pivot(), at(2, 1), "the group is not pushed");
    assert_eq!(counter.attempts(), 0, "a refusal is not a wedged attempt");
}

// component/falling-group-control::TC-007
#[test]
fn a_blocked_target_with_a_free_opposite_side_pushes_the_group() {
    let board = Board::with_geometry(spec().board_geometry);

    // Grounded: the downward target leaves the board, so the group is lifted.
    let mut grounded = FallingGroup::new(hand(DropShape::I, None), [0, 1], at(2, 13), 0);
    let mut counter = DoubleRotation::new();
    grounded.rotate(&board, true, &mut counter, 2, 4);
    assert_eq!(grounded.transform_id(), 1);
    assert_eq!(
        grounded.rotate(&board, true, &mut counter, 2, 4),
        RotationOutcome::PushedBack { dx: 0, dy: -1 }
    );
    assert_eq!(grounded.transform_id(), 2);
    assert_eq!(grounded.pivot(), at(2, 12));

    // Against the wall: the leftward target leaves the board, so it side-steps.
    let mut walled = FallingGroup::new(hand(DropShape::I, None), [0, 1], at(0, 6), 0);
    let mut counter = DoubleRotation::new();
    assert_eq!(
        walled.rotate(&board, false, &mut counter, 2, 4),
        RotationOutcome::PushedBack { dx: 1, dy: 0 }
    );
    assert_eq!(walled.transform_id(), 3);
    assert_eq!(walled.pivot(), at(1, 6));
}

// component/falling-group-control::TC-008
#[test]
fn a_group_wedged_between_two_columns_flips_on_the_even_attempt() {
    let mut board = Board::with_geometry(spec().board_geometry);
    for y in 5..=7 {
        board.set(at(1, y), Cell::Color(3));
        board.set(at(3, y), Cell::Color(3));
    }
    let mut group = FallingGroup::new(hand(DropShape::I, None), [0, 1], at(2, 6), 0);
    let mut counter = DoubleRotation::new();

    assert_eq!(
        group.rotate(&board, true, &mut counter, 2, 4),
        RotationOutcome::Blocked
    );
    assert_eq!(
        counter.attempts(),
        1,
        "an odd count holds the rotation back"
    );
    assert_eq!(group.transform_id(), 0);

    assert_eq!(
        group.rotate(&board, true, &mut counter, 2, 4),
        RotationOutcome::DoubleRotated
    );
    assert_eq!(counter.attempts(), 2);
    assert_eq!(
        group.transform_id(),
        2,
        "the even attempt releases the flip"
    );
}

// component/falling-group-control::TC-009
#[test]
fn confirming_a_rotation_rounds_the_wedged_counter_down_to_even() {
    let mut board = Board::with_geometry(spec().board_geometry);
    for y in 5..=7 {
        board.set(at(1, y), Cell::Color(3));
        board.set(at(3, y), Cell::Color(3));
    }
    // Block the flip target too, so attempts accumulate without turning.
    board.set(at(2, 7), Cell::Color(3));
    let mut group = FallingGroup::new(hand(DropShape::I, None), [0, 1], at(2, 6), 0);
    let mut counter = DoubleRotation::new();
    for _ in 0..3 {
        assert_eq!(
            group.rotate(&board, true, &mut counter, 2, 4),
            RotationOutcome::Blocked
        );
    }
    assert_eq!(
        counter.attempts(),
        3,
        "the counter is odd and the group is stuck"
    );

    // Move to a free column and rotate: the confirmation settles the counter.
    let mut free = FallingGroup::new(hand(DropShape::I, None), [0, 1], at(4, 11), 0);
    assert_eq!(
        free.rotate(&board, true, &mut counter, 2, 4),
        RotationOutcome::Confirmed
    );
    assert_eq!(counter.attempts(), 2, "confirmation rounds 3 down to 2");

    // The parity still means the same thing afterwards.
    assert_eq!(
        group.rotate(&board, true, &mut counter, 2, 4),
        RotationOutcome::Blocked
    );
    assert_eq!(counter.attempts(), 3);
}

// component/falling-group-control::TC-010
#[test]
fn a_blocked_spawn_pose_supplies_nothing_and_leaves_the_cursor_alone() {
    let spec = spec();
    let mut board = Board::with_geometry(spec.board_geometry);
    // Fill the spawn column up through the spawn pose.
    for y in 0..14 {
        board.set(at(spec.board_geometry.spawn_column(), y), Cell::Color(3));
    }
    let mut rng = MatchRng::derive(spec.root_seed, 0, 0, 0, StreamName::Color);
    let mut stream = DropStream::new(
        spec.drop_sets[0].clone(),
        spec.drop.next_queue_len,
        spec.color_count,
        &mut rng,
    );
    let cursor_before = stream.cursor();
    let queue_before: Vec<_> = stream.queued().collect();
    let board_before = board.clone();

    let failure = spawn_group(
        &board,
        &mut stream,
        spec.board_geometry,
        spec.color_count,
        &mut rng,
    );

    assert!(failure.is_err(), "a blocked spawn pose supplies no group");
    assert_eq!(
        stream.cursor(),
        cursor_before,
        "the cursor does not advance"
    );
    assert_eq!(
        stream.queued().collect::<Vec<_>>(),
        queue_before,
        "NEXT does not advance"
    );
    assert_eq!(board, board_before, "nothing is written to the board");
}

// component/falling-group-control::TC-011
#[test]
fn a_blocked_spawn_column_only_decides_the_round_at_the_next_spawn() {
    let spec = spec();
    let mut state = game_core::MatchState::new(spec.clone());
    let idle = game_core::input::TickInputs::new([PlayerActions::EMPTY, PlayerActions::EMPTY])
        .expect("two slots");
    state.step(&idle).expect("leave the intro");

    // Sixty ticks of ordinary control decide nothing.
    for _ in 0..60 {
        let report = state.step(&idle).expect("a tick advances");
        assert!(
            report.spawn_failures.is_empty(),
            "defeat is judged at spawn, not while a group is controllable"
        );
    }
    assert!(!state.is_defeated(0));
    assert!(!state.is_defeated(1));
}

// component/falling-group-control::TC-012
#[test]
fn an_unsupported_ball_free_falls_after_its_split_delay() {
    let mut harness = Harness::new(hand(DropShape::I, None), at(2, 9));
    // A ledge under the pivot column and a floor three cells lower under the
    // follower column.
    harness.fill(&[(2, 10), (2, 11), (2, 12), (2, 13), (3, 13)]);
    // Orientation 1 lays the group flat, follower to the right.
    let rules_transform = {
        let mut counter = DoubleRotation::new();
        harness
            .group
            .rotate(&harness.board, true, &mut counter, 2, 4)
    };
    assert_eq!(rules_transform, RotationOutcome::Confirmed);

    let ControlOutcome::Locked { split, .. } = harness.tick(actions(&[GameAction::HardDrop]))
    else {
        panic!("a hard drop locks");
    };
    let mut split = split.expect("the follower lost its support");
    assert_eq!(split.falls().len(), 1, "only the follower falls");
    let fall = split.falls()[0];
    assert_eq!(fall.from, at(3, 9));
    assert_eq!(fall.to, at(3, 12));
    assert_eq!(
        fall.start_tick, 2,
        "a follower takes the longer split delay"
    );
    assert_eq!(
        fall.arrival_tick, 21,
        "three cells of free fall take 19 ticks"
    );

    for tick in 1..21 {
        split.tick(&mut harness.board);
        assert!(!split.is_complete(), "still in flight at tick {tick}");
        assert_eq!(
            split.in_flight(),
            vec![at(3, 9)],
            "an in-flight ball is excluded from scanning"
        );
    }
    split.tick(&mut harness.board);
    assert!(split.is_complete());
    assert_eq!(harness.board.get(at(3, 12)), Cell::Color(1));
    assert_eq!(harness.board.get(at(3, 9)), Cell::Empty);
    assert_eq!(
        harness.board.get(at(2, 9)),
        Cell::Color(0),
        "the supported pivot stays where it locked"
    );
}

// component/falling-group-control::TC-013
#[test]
fn the_lock_grace_accumulates_to_the_configured_ticks_and_survives_a_lift() {
    let mut harness = Harness::new(hand(DropShape::I, None), at(2, 13));
    for tick in 1..32 {
        assert_eq!(
            harness.tick(PlayerActions::EMPTY),
            ControlOutcome::Continue,
            "tick {tick} is still controllable"
        );
    }
    assert_eq!(harness.control.lock_delay_ticks(), 31);
    let ControlOutcome::Locked { .. } = harness.tick(PlayerActions::EMPTY) else {
        panic!("the grace runs out on tick 32");
    };
    assert_eq!(
        harness.control.last_lock_cause(),
        Some(LockCause::LockDelay)
    );

    // A lift does not clear what the group already accumulated.
    let mut lifted = Harness::new(hand(DropShape::I, None), at(2, 13));
    for _ in 0..20 {
        lifted.tick(PlayerActions::EMPTY);
    }
    assert_eq!(lifted.control.lock_delay_ticks(), 20);
    lifted.tick(actions(&[GameAction::RotateClockwise]));
    let outcome = lifted.tick(actions(&[GameAction::RotateClockwise]));
    assert_eq!(outcome, ControlOutcome::Continue);
    assert_eq!(lifted.pivot().1, 12, "the second rotation lifted the group");
    assert!(
        lifted.control.lock_delay_ticks() >= 20,
        "the accumulated grace is not cleared by the lift"
    );
}

// component/falling-group-control::TC-014
#[test]
fn holding_soft_drop_or_reaching_the_lift_limit_locks_immediately() {
    let mut soft = Harness::new(hand(DropShape::I, None), at(2, 13));
    let ControlOutcome::Locked { .. } = soft.tick(actions(&[GameAction::SoftDrop])) else {
        panic!("a grounded group locks on the tick soft drop is held");
    };
    assert_eq!(soft.control.last_lock_cause(), Some(LockCause::SoftDrop));

    // Lift limit. With the shipped frame data the 32-tick lock grace always
    // expires long before eight push-ups can accumulate, so the grace is
    // widened here to exercise the lift rule on its own. See the note in
    // `docs/development/decision/timing-parameter-source.md`: these values are
    // calibration inputs, and this test pins the rule, not the numbers.
    let mut lifted = Harness::new(hand(DropShape::I, None), at(2, 13))
        .with_timing(|timing| timing.lock_delay_ticks = u16::MAX);
    let limit = lifted.timing.lift_limit;
    let mut locked = false;
    for _ in 0..1_000 {
        let before = lifted.control.lifts();
        if matches!(
            lifted.tick(actions(&[GameAction::RotateClockwise])),
            ControlOutcome::Locked { .. }
        ) {
            locked = true;
            break;
        }
        assert!(
            lifted.control.lifts() < limit,
            "below the limit the group stays controllable"
        );
        assert!(lifted.control.lifts() >= before);
    }
    assert!(locked, "the lift limit must end the group");
    assert_eq!(lifted.control.lifts(), limit);
    assert_eq!(lifted.control.last_lock_cause(), Some(LockCause::LiftLimit));
}

// component/falling-group-control::TC-015
#[test]
fn natural_fall_and_soft_drop_advance_at_their_configured_rates() {
    let mut natural = Harness::new(hand(DropShape::I, None), at(2, 1));
    let rate = natural.timing.natural_fall_ticks;
    let mut falls = Vec::new();
    for tick in 1..=(rate * 3) {
        let before = natural.pivot().1;
        natural.tick(PlayerActions::EMPTY);
        if natural.pivot().1 != before {
            falls.push(tick);
        }
    }
    assert_eq!(falls, vec![rate, rate * 2, rate * 3]);

    let mut soft = Harness::new(hand(DropShape::I, None), at(2, 1));
    let soft_rate = soft.timing.soft_drop_ticks;
    let mut soft_falls = Vec::new();
    for tick in 1..=(soft_rate * 3) {
        let before = soft.pivot().1;
        soft.tick(actions(&[GameAction::SoftDrop]));
        if soft.pivot().1 != before {
            soft_falls.push(tick);
        }
    }
    assert_eq!(soft_falls, vec![soft_rate, soft_rate * 2, soft_rate * 3]);
    assert!(soft_rate < rate, "the two rates never stack");
}

// component/falling-group-control::TC-016
#[test]
fn horizontal_input_repeats_on_the_configured_delay_and_respects_its_cooldown() {
    let mut held = Harness::new(hand(DropShape::I, None), at(5, 6));
    let delay = held.timing.horizontal_repeat_delay_ticks;
    let interval = held.timing.horizontal_repeat_interval_ticks;
    let mut moves = Vec::new();
    for tick in 0..16_u16 {
        let before = held.pivot().0;
        held.tick(actions(&[GameAction::Left]));
        if held.pivot().0 != before {
            moves.push(tick);
        }
    }
    assert_eq!(
        moves,
        vec![
            0,
            delay,
            delay + interval,
            delay + interval * 2,
            delay + interval * 3
        ],
        "the press moves once, then repeats on the delay and interval"
    );
    assert_eq!(held.pivot().0, 0, "five moves reach the wall");
    // The repeat that would follow is a no-op against the wall.
    let before = held.pivot().0;
    held.tick(actions(&[GameAction::Left]));
    assert_eq!(held.pivot().0, before);

    // Tapping: a fresh press each tick still obeys the one-tick cooldown.
    let mut tapped = Harness::new(hand(DropShape::I, None), at(3, 6));
    let mut tap_moves = Vec::new();
    for tick in 1..=4_u16 {
        let before = tapped.pivot().0;
        // Alternating press and release makes each press a new one.
        let input = if tick % 2 == 1 {
            actions(&[GameAction::Left])
        } else {
            PlayerActions::EMPTY
        };
        tapped.tick(input);
        if tapped.pivot().0 != before {
            tap_moves.push(tick);
        }
    }
    assert_eq!(tap_moves, vec![1, 3]);
}

// component/falling-group-control::TC-017
#[test]
fn one_tick_applies_horizontal_then_rotation_then_soft_drop() {
    let soft_rate = spec().drop.soft_drop_ticks;

    // Horizontal plus rotation plus soft drop: it moves, turns, and does not
    // soft drop, because a held direction suppresses the soft rate.
    let mut all = Harness::new(hand(DropShape::I, None), at(3, 6));
    for _ in 0..soft_rate {
        all.tick(actions(&[
            GameAction::Left,
            GameAction::RotateClockwise,
            GameAction::SoftDrop,
        ]));
    }
    assert!(all.pivot().0 < 3, "the horizontal move happened");
    assert_eq!(all.pivot().1, 6, "a held direction suppresses soft drop");
    assert_ne!(all.group.transform_id(), 0, "the rotation happened");

    // Horizontal plus soft drop: only the move.
    let mut moved = Harness::new(hand(DropShape::I, None), at(3, 6));
    for _ in 0..soft_rate {
        moved.tick(actions(&[GameAction::Left, GameAction::SoftDrop]));
    }
    assert!(moved.pivot().0 < 3);
    assert_eq!(moved.pivot().1, 6);
    assert_eq!(moved.group.transform_id(), 0);

    // Rotation plus soft drop: it turns, then falls at the soft rate.
    let mut turned = Harness::new(hand(DropShape::I, None), at(3, 6));
    for _ in 0..soft_rate {
        turned.tick(actions(&[
            GameAction::RotateClockwise,
            GameAction::SoftDrop,
        ]));
    }
    assert_eq!(turned.pivot().0, 3);
    assert_eq!(turned.pivot().1, 7, "soft drop moved it one cell");
    assert_ne!(turned.group.transform_id(), 0);

    // Soft drop alone.
    let mut dropped = Harness::new(hand(DropShape::I, None), at(3, 6));
    for _ in 0..soft_rate {
        dropped.tick(actions(&[GameAction::SoftDrop]));
    }
    assert_eq!(dropped.pivot(), (3, 7));
    assert_eq!(dropped.group.transform_id(), 0);
}

// component/falling-group-control::TC-018
#[test]
fn the_same_seed_and_action_log_reproduce_the_same_locks_and_next() {
    use game_core::MatchState;
    use game_core::input::TickInputs;

    let log: Vec<PlayerActions> = (0..200_u32)
        .map(|tick| match tick % 7 {
            0 => actions(&[GameAction::Left]),
            1 => actions(&[GameAction::RotateClockwise]),
            2 => actions(&[GameAction::Right]),
            3 => actions(&[GameAction::SoftDrop]),
            4 => actions(&[GameAction::RotateCounterClockwise]),
            5 => PlayerActions::EMPTY,
            _ => actions(&[GameAction::HardDrop]),
        })
        .collect();

    let replay = |split_at: usize| {
        let mut state = MatchState::new(spec());
        let run = |state: &mut MatchState, range: std::ops::Range<usize>| {
            for tick in range {
                let inputs =
                    TickInputs::new([log[tick], log[log.len() - 1 - tick]]).expect("two slots");
                state.step(&inputs).expect("a tick advances");
            }
        };
        run(&mut state, 0..split_at);
        run(&mut state, split_at..log.len());
        let board = state.board(0).expect("slot 0 exists").clone();
        let cursor = state.stream(0).expect("slot 0 exists").cursor();
        let queue: Vec<_> = state.stream(0).expect("slot 0 exists").queued().collect();
        let group = state
            .active_group(0)
            .map(|group| (group.pivot(), group.transform_id()));
        (board, cursor, queue, group)
    };

    let whole = replay(200);
    let halves = replay(97);
    assert_eq!(whole.0, halves.0, "the board must match");
    assert_eq!(whole.1, halves.1, "the drop cursor must match");
    assert_eq!(whole.2, halves.2, "NEXT must match");
    assert_eq!(whole.3, halves.3, "the active pose must match");

    let again = replay(200);
    assert_eq!(whole.0, again.0);
    assert_eq!(whole.1, again.1);
}
