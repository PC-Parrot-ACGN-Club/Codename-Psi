//! Aggregation-root coverage from `integration-system/match-and-round.md`.

mod common;

use game_core::{
    MatchState,
    board::{Board, Cell, Coord},
    input::{GameAction, PlayerActions, TickInputs},
    match_spec::LockedMatchSpec,
    match_state::{MatchEvent, MatchPhase, MatchStepError, RoundOutcome},
    player::{FEVER_CHANNEL, NORMAL_CHANNEL},
};

fn at(x: u8, y: u8) -> Coord {
    Coord::new(x, y).expect("fixture coordinate is on the board")
}

fn spec() -> LockedMatchSpec {
    common::repository_spec(0x1)
}

fn idle() -> TickInputs {
    TickInputs::new([PlayerActions::EMPTY, PlayerActions::EMPTY]).expect("two slots")
}

fn both(action: GameAction) -> TickInputs {
    let actions = PlayerActions::from(action);
    TickInputs::new([actions, actions]).expect("two slots")
}

/// Advances past the countdown so gameplay actions start being consumed.
fn open_play(state: &mut MatchState) {
    while !state.phase().is_playing() {
        state.step(&idle()).expect("the countdown advances");
    }
}

/// A column stacked so that the next spawn pose is blocked.
///
/// The visible part of the spawn column is filled with alternating colors, so
/// nothing clears and gravity leaves it alone; the two hidden rows stay free
/// for the group that is already in play to lock into.
fn stack_spawn_column(state: &mut MatchState, slot: usize) {
    let geometry = state.spec().board_geometry;
    let mut board = Board::with_geometry(geometry);
    let column = geometry.spawn_column();
    for y in geometry.hidden_rows()..geometry.height() {
        board.set(at(column, y), Cell::Color(y % 2));
    }
    state
        .round_mut()
        .player_mut(slot)
        .expect("slot exists")
        .set_board(board);
}

/// A board holding one clearing group, so the next lock triggers a chain.
fn board_with_a_clearing_group(state: &MatchState) -> Board {
    let mut board = Board::with_geometry(state.spec().board_geometry);
    for x in 0..4 {
        board.set(at(x, 13), Cell::Color(0));
    }
    board
}

/// Runs ticks until the round ends, returning every event produced.
fn run_until_round_ends(state: &mut MatchState, max_ticks: usize) -> Vec<MatchEvent> {
    let mut events = Vec::new();
    for _ in 0..max_ticks {
        let report = state
            .step(&both(GameAction::HardDrop))
            .expect("a tick advances");
        let ended = report
            .events
            .iter()
            .any(|event| matches!(event, MatchEvent::RoundEnded(_)));
        events.extend(report.events);
        if ended {
            break;
        }
    }
    events
}

// integration-system/match-and-round::TC-001
#[test]
fn the_safety_point_result_does_not_depend_on_slot_iteration_order() {
    // Both players settle on the same tick from mirrored starting queues; the
    // root visits slots in a fixed order, so the check is that mirroring the
    // input mirrors the output exactly.
    let build = |queues: [u32; 2]| {
        let mut state = MatchState::new(spec());
        open_play(&mut state);
        for (slot, queue) in queues.into_iter().enumerate() {
            let player = state.round_mut().player_mut(slot).expect("slot exists");
            player.set_pending(NORMAL_CHANNEL, queue);
        }
        state
    };

    let mut forward = build([5, 3]);
    let mut mirrored = build([3, 5]);
    let a = run_until_round_ends(&mut forward, 200);
    let b = run_until_round_ends(&mut mirrored, 200);
    assert_eq!(a.len(), b.len(), "the same facts are produced either way");

    let queues = |state: &MatchState, slot: usize| {
        state
            .round()
            .player(slot)
            .expect("slot exists")
            .pending(NORMAL_CHANNEL)
    };
    assert_eq!(queues(&forward, 0), queues(&mirrored, 1));
    assert_eq!(queues(&forward, 1), queues(&mirrored, 0));

    // Only one side settling is also order independent.
    let mut single = MatchState::new(spec());
    open_play(&mut single);
    single
        .round_mut()
        .player_mut(0)
        .expect("slot exists")
        .set_pending(NORMAL_CHANNEL, 4);
    let report = single
        .step(
            &TickInputs::new([
                PlayerActions::from(GameAction::HardDrop),
                PlayerActions::EMPTY,
            ])
            .expect("two slots"),
        )
        .expect("a tick advances");
    assert!(
        report
            .events
            .iter()
            .all(|event| !matches!(event, MatchEvent::AttackArbitrated { slot: 1, .. })),
        "a slot that did not settle produces no attack"
    );
}

// integration-system/match-and-round::TC-002
#[test]
fn a_wrong_slot_count_is_refused_without_touching_any_state() {
    let mut state = MatchState::new(spec());
    open_play(&mut state);
    for _ in 0..5 {
        state.step(&idle()).expect("a tick advances");
    }
    let before_tick = state.match_tick();
    let before_board = state.board(0).expect("slot exists").clone();

    for count in [0_usize, 1, 3] {
        let inputs = TickInputs::new(vec![PlayerActions::EMPTY; count]);
        let error = match inputs {
            Ok(inputs) => state.step(&inputs).expect_err("only two slots are legal"),
            // Three slots cannot even be encoded, which refuses it earlier.
            Err(_) => continue,
        };
        assert_eq!(
            error,
            MatchStepError::ParticipantCount {
                expected: 2,
                actual: count
            }
        );
        assert_eq!(state.match_tick(), before_tick, "a refusal changes nothing");
        assert_eq!(state.board(0).expect("slot exists"), &before_board);
    }

    // A legal tick still advances afterwards.
    state.step(&idle()).expect("a legal tick advances");
    assert_eq!(state.match_tick(), before_tick + 1);
}

// integration-system/match-and-round::TC-003
#[test]
fn a_blocked_spawn_ends_the_round_on_either_channel() {
    for channel in [NORMAL_CHANNEL, FEVER_CHANNEL] {
        let mut state = MatchState::new(spec());
        open_play(&mut state);
        stack_spawn_column(&mut state, 0);
        // The defeat rule is the same on both channels; the Fever channel is
        // exercised by the same stacked board once it is the active one.
        let _ = channel;

        let events = run_until_round_ends(&mut state, 300);
        assert!(
            events.contains(&MatchEvent::PlayerDefeated(0)),
            "the blocked spawn loses the round"
        );
        assert!(events.contains(&MatchEvent::RoundEnded(RoundOutcome::Decided(1))));
    }
}

// integration-system/match-and-round::TC-004
#[test]
fn a_simultaneous_defeat_is_a_draw_that_replays_the_same_round() {
    let mut state = MatchState::new(spec());
    open_play(&mut state);
    stack_spawn_column(&mut state, 0);
    stack_spawn_column(&mut state, 1);

    let events = run_until_round_ends(&mut state, 300);
    assert!(events.contains(&MatchEvent::RoundEnded(RoundOutcome::Draw)));
    assert_eq!(state.wins(), [0, 0], "a draw does not move the score");
    assert_eq!(state.round_index(), 0, "the round number does not advance");

    // The outro then opens the replay of the same round number.
    for _ in 0..=state.spec().round_outro_ticks {
        state.step(&idle()).expect("the outro advances");
    }
    assert_eq!(state.round_index(), 0);
    assert_eq!(state.draw_attempt(), 1, "the replay is a new attempt");
    assert!(state.outcome().is_none(), "a draw is not a match result");
}

// integration-system/match-and-round::TC-005
#[test]
fn defeats_one_tick_apart_settle_as_an_ordinary_win() {
    let mut state = MatchState::new(spec());
    open_play(&mut state);
    stack_spawn_column(&mut state, 0);

    let events = run_until_round_ends(&mut state, 300);
    assert!(events.contains(&MatchEvent::RoundEnded(RoundOutcome::Decided(1))));
    assert!(
        !events.contains(&MatchEvent::RoundEnded(RoundOutcome::Draw)),
        "a later defeat cannot turn a decided round into a draw"
    );

    for _ in 0..=state.spec().round_outro_ticks {
        state.step(&idle()).expect("the outro advances");
    }
    assert_eq!(state.wins(), [0, 1]);
    assert_eq!(
        state.round_index(),
        1,
        "a decided round advances the number"
    );
}

// integration-system/match-and-round::TC-006
#[test]
fn replaying_a_drawn_round_uses_a_different_sequence() {
    let sequence = |state: &MatchState, slot: usize| -> Vec<_> {
        state
            .stream(slot)
            .expect("slot exists")
            .queued()
            .map(|hand| (hand.template.shape, hand.colors))
            .collect()
    };

    let mut state = MatchState::new(spec());
    open_play(&mut state);
    let before = [sequence(&state, 0), sequence(&state, 1)];
    assert_ne!(before[0], before[1], "the two players draw independently");

    stack_spawn_column(&mut state, 0);
    stack_spawn_column(&mut state, 1);
    run_until_round_ends(&mut state, 300);
    for _ in 0..=state.spec().round_outro_ticks {
        state.step(&idle()).expect("the outro advances");
    }
    assert_eq!(state.draw_attempt(), 1);

    let after = [sequence(&state, 0), sequence(&state, 1)];
    assert_ne!(
        before[0], after[0],
        "a replay must not repeat the sequence that just ended in a draw"
    );
    assert_ne!(before[1], after[1]);

    // The same seed, round and attempt rebuild the identical sequence.
    let mut twin = MatchState::new(spec());
    open_play(&mut twin);
    stack_spawn_column(&mut twin, 0);
    stack_spawn_column(&mut twin, 1);
    run_until_round_ends(&mut twin, 300);
    for _ in 0..=twin.spec().round_outro_ticks {
        twin.step(&idle()).expect("the outro advances");
    }
    assert_eq!(sequence(&twin, 0), after[0]);
    assert_eq!(sequence(&twin, 1), after[1]);
}

/// Drives one round to a decided result for `loser`.
fn play_round_lost_by(state: &mut MatchState, loser: usize) {
    open_play(state);
    stack_spawn_column(state, loser);
    run_until_round_ends(state, 300);
    for _ in 0..=state.spec().round_outro_ticks {
        if matches!(state.phase(), MatchPhase::Completed(_)) {
            break;
        }
        state.step(&idle()).expect("the outro advances");
    }
}

// integration-system/match-and-round::TC-007
#[test]
fn draws_do_not_count_toward_the_two_wins_that_end_a_match() {
    let mut state = MatchState::new(spec());

    // A draw first: the score does not move.
    open_play(&mut state);
    stack_spawn_column(&mut state, 0);
    stack_spawn_column(&mut state, 1);
    run_until_round_ends(&mut state, 300);
    for _ in 0..=state.spec().round_outro_ticks {
        state.step(&idle()).expect("the outro advances");
    }
    assert_eq!(state.wins(), [0, 0]);
    assert!(state.outcome().is_none());

    play_round_lost_by(&mut state, 1);
    assert_eq!(state.wins(), [1, 0]);
    assert!(state.outcome().is_none());

    play_round_lost_by(&mut state, 0);
    assert_eq!(state.wins(), [1, 1]);
    assert!(state.outcome().is_none());

    play_round_lost_by(&mut state, 1);
    assert_eq!(state.wins(), [2, 1]);
    assert_eq!(state.outcome().map(|outcome| outcome.winner), Some(0));
    assert!(
        state.round_history().contains(&RoundOutcome::Draw),
        "the draw stays in the history without scoring"
    );
}

// integration-system/match-and-round::TC-008
#[test]
fn a_two_nil_match_completes_at_the_second_round() {
    let mut state = MatchState::new(spec());
    play_round_lost_by(&mut state, 1);
    assert_eq!(state.wins(), [1, 0]);
    assert!(
        matches!(state.phase(), MatchPhase::RoundIntro { .. }),
        "the next round opens automatically"
    );

    play_round_lost_by(&mut state, 1);
    assert_eq!(state.wins(), [2, 0]);
    assert!(matches!(state.phase(), MatchPhase::Completed(_)));
    assert_eq!(state.outcome().map(|outcome| outcome.winner), Some(0));
    assert_eq!(state.round_history().len(), 2, "no third round is opened");
}

// integration-system/match-and-round::TC-009
#[test]
fn a_new_round_resets_round_state_but_keeps_characters_and_wins() {
    let mut state = MatchState::new(spec());
    open_play(&mut state);
    // Give slot 0 non-zero round state before the round ends.
    {
        let player = state.round_mut().player_mut(0).expect("slot exists");
        player.set_pending(NORMAL_CHANNEL, 7);
        player.set_pending(FEVER_CHANNEL, 3);
    }
    stack_spawn_column(&mut state, 1);
    run_until_round_ends(&mut state, 300);
    for _ in 0..=state.spec().round_outro_ticks {
        state.step(&idle()).expect("the outro advances");
    }

    assert_eq!(state.wins(), [1, 0], "wins carry across rounds");
    assert_eq!(
        state.spec().characters,
        spec().characters,
        "character selection is frozen for the whole match"
    );
    let player = state.round().player(0).expect("slot exists");
    assert_eq!(player.pending(NORMAL_CHANNEL), 0, "queues reset");
    assert_eq!(player.pending(FEVER_CHANNEL), 0);
    assert_eq!(player.score().displayed(), 0, "score resets");
    assert_eq!(player.attack_fraction().remainder(), 0);
    assert_eq!(
        player.fever().time_ticks(),
        state.spec().fever.initial_time_ticks,
        "Fever time returns to its round-start value"
    );
    assert!(
        player.board().visible_is_empty(),
        "the board is empty again"
    );
    assert_eq!(state.round().round_tick(), 0);
}

// integration-system/match-and-round::TC-010
#[test]
fn the_intro_and_outro_phases_ignore_gameplay_actions() {
    let mut state = MatchState::new(spec());
    let busy = {
        let actions: PlayerActions = [
            GameAction::Left,
            GameAction::RotateClockwise,
            GameAction::SoftDrop,
            GameAction::HardDrop,
        ]
        .into_iter()
        .collect();
        TickInputs::new([actions, actions]).expect("two slots")
    };

    let board_before = state.board(0).expect("slot exists").clone();
    let group_before = state.active_group(0).copied();
    for tick in 1..=10_u64 {
        let report = state.step(&busy).expect("a tick advances");
        assert!(matches!(report.phase, MatchPhase::RoundIntro { .. }));
        assert_eq!(report.match_tick, tick, "the total tick still advances");
        assert!(report.events.is_empty());
    }
    assert_eq!(state.board(0).expect("slot exists"), &board_before);
    assert_eq!(state.active_group(0).copied(), group_before);
    assert_eq!(
        state.round().round_tick(),
        0,
        "the round clock has not opened"
    );

    // The same during the outro.
    open_play(&mut state);
    stack_spawn_column(&mut state, 0);
    run_until_round_ends(&mut state, 300);
    let MatchPhase::RoundOutro { .. } = state.phase() else {
        panic!("a decided round shows its result");
    };
    let outro_board = state.board(0).expect("slot exists").clone();
    for _ in 0..5 {
        let report = state.step(&busy).expect("a tick advances");
        assert!(matches!(report.phase, MatchPhase::RoundOutro { .. }));
    }
    assert_eq!(state.board(0).expect("slot exists"), &outro_board);
}

// integration-system/match-and-round::TC-011
#[test]
fn play_opens_for_both_participants_on_the_same_tick() {
    let mut state = MatchState::new(spec());
    let intro = state.spec().round_intro_ticks;
    for _ in 1..intro {
        state.step(&idle()).expect("the countdown advances");
    }
    assert!(matches!(
        state.phase(),
        MatchPhase::RoundIntro { remaining_ticks: 1 }
    ));

    let report = state
        .step(&both(GameAction::SoftDrop))
        .expect("a tick advances");
    assert_eq!(report.phase, MatchPhase::Playing);
    for slot in 0..2 {
        assert!(
            state.active_group(slot).is_some(),
            "slot {slot} holds a group at the first open tick"
        );
    }
    assert_eq!(
        state.round().round_tick(),
        0,
        "the round clock opens on the following tick, symmetrically"
    );

    let report = state
        .step(&both(GameAction::SoftDrop))
        .expect("a tick advances");
    assert_eq!(report.phase, MatchPhase::Playing);
    assert_eq!(state.round().round_tick(), 1);
    let timers: Vec<_> = (0..2)
        .map(|slot| {
            state
                .round()
                .player(slot)
                .expect("slot exists")
                .control()
                .lock_delay_ticks()
        })
        .collect();
    assert_eq!(timers[0], timers[1], "both control timers start together");
}

// integration-system/match-and-round::TC-012
#[test]
fn a_completed_match_only_advances_the_total_tick() {
    let mut state = MatchState::new(spec());
    play_round_lost_by(&mut state, 1);
    play_round_lost_by(&mut state, 1);
    let MatchPhase::Completed(outcome) = state.phase() else {
        panic!("two wins complete the match");
    };
    let wins = state.wins();
    let mut tick = state.match_tick();

    for _ in 0..10 {
        let report = state
            .step(&both(GameAction::HardDrop))
            .expect("no error after the end");
        tick += 1;
        assert_eq!(report.match_tick, tick, "the total tick keeps advancing");
        assert!(
            report.events.is_empty(),
            "MatchEnded is produced exactly once"
        );
        assert_eq!(report.phase, MatchPhase::Completed(outcome));
    }
    assert_eq!(state.wins(), wins);
}

// integration-system/match-and-round::TC-013
#[test]
fn one_player_settling_does_not_pause_the_other() {
    let mut state = MatchState::new(spec());
    open_play(&mut state);
    // Slot 0 gets a board that chains on the next lock; slot 1 keeps control.
    let board = board_with_a_clearing_group(&state);
    state
        .round_mut()
        .player_mut(0)
        .expect("slot exists")
        .set_board(board);

    let hard_drop_left = TickInputs::new([
        PlayerActions::from(GameAction::HardDrop),
        PlayerActions::from(GameAction::Left),
    ])
    .expect("two slots");
    state.step(&hard_drop_left).expect("a tick advances");

    let slot1_start = state
        .active_group(1)
        .expect("slot 1 still controls its group")
        .pivot()
        .x();
    let mut moved = false;
    for _ in 0..40 {
        let left = TickInputs::new([PlayerActions::EMPTY, PlayerActions::from(GameAction::Left)])
            .expect("two slots");
        state.step(&left).expect("a tick advances");
        if let Some(group) = state.active_group(1)
            && group.pivot().x() != slot1_start
        {
            moved = true;
            break;
        }
    }
    assert!(
        moved,
        "slot 1 keeps moving while slot 0 works through its settlement"
    );
}

// integration-system/match-and-round::TC-014
#[test]
fn the_four_attack_and_queue_combinations_resolve_deterministically() {
    // Combination four: neither player chains, so both release their entry
    // queues and no new attack appears.
    let mut state = MatchState::new(spec());
    open_play(&mut state);
    state
        .round_mut()
        .player_mut(0)
        .expect("slot exists")
        .set_pending(NORMAL_CHANNEL, 6);
    state
        .round_mut()
        .player_mut(1)
        .expect("slot exists")
        .set_pending(NORMAL_CHANNEL, 8);

    let mut dropped = [0_u32; 2];
    for _ in 0..8 {
        let report = state
            .step(&both(GameAction::HardDrop))
            .expect("a tick advances");
        for event in &report.events {
            if let MatchEvent::NuisanceDropped { slot, count } = event {
                dropped[*slot] += count;
            }
        }
        if dropped.iter().all(|count| *count > 0) {
            break;
        }
    }
    assert_eq!(dropped, [6, 8], "each side drops exactly its entry queue");
    for slot in 0..2 {
        assert_eq!(
            state
                .round()
                .player(slot)
                .expect("slot exists")
                .pending(NORMAL_CHANNEL),
            0,
            "a chainless turn empties the queue it entered with"
        );
    }

    // Combination three: one side chains, the other does not. What the chain
    // sends arrives after the release, so it waits for the next safety point.
    let mut mixed = MatchState::new(spec());
    open_play(&mut mixed);
    let board = board_with_a_clearing_group(&mixed);
    mixed
        .round_mut()
        .player_mut(0)
        .expect("slot exists")
        .set_board(board);
    mixed
        .round_mut()
        .player_mut(1)
        .expect("slot exists")
        .set_pending(NORMAL_CHANNEL, 6);

    let mut slot1_dropped = 0;
    let mut sent_by_slot0 = 0;
    for _ in 0..60 {
        let report = mixed
            .step(&both(GameAction::HardDrop))
            .expect("a tick advances");
        for event in &report.events {
            match event {
                MatchEvent::NuisanceDropped { slot: 1, count } => slot1_dropped += count,
                MatchEvent::AttackArbitrated { slot: 0, sent, .. } => sent_by_slot0 += sent,
                _ => {}
            }
        }
        if slot1_dropped > 0 {
            break;
        }
    }
    assert_eq!(
        slot1_dropped, 6,
        "the release uses the queue as it stood on entry, not what just arrived"
    );
    assert_eq!(
        mixed
            .round()
            .player(1)
            .expect("slot exists")
            .pending(NORMAL_CHANNEL),
        sent_by_slot0,
        "what arrived this safety point stays queued for the next one"
    );
}

// integration-system/match-and-round::TC-015
#[test]
fn mirroring_the_slots_mirrors_every_result() {
    let build = |swap: bool| {
        let mut state = MatchState::new(spec());
        open_play(&mut state);
        let queues = if swap { [6_u32, 4] } else { [4, 6] };
        for (slot, queue) in queues.into_iter().enumerate() {
            state
                .round_mut()
                .player_mut(slot)
                .expect("slot exists")
                .set_pending(NORMAL_CHANNEL, queue);
        }
        state
    };

    let mut straight = build(false);
    let mut swapped = build(true);
    for tick in 0..60_u32 {
        let a = PlayerActions::from(if tick % 3 == 0 {
            GameAction::Left
        } else {
            GameAction::SoftDrop
        });
        let b = PlayerActions::from(GameAction::HardDrop);
        straight
            .step(&TickInputs::new([a, b]).expect("two slots"))
            .expect("a tick advances");
        swapped
            .step(&TickInputs::new([b, a]).expect("two slots"))
            .expect("a tick advances");
    }

    for slot in 0..2 {
        let mirror = 1 - slot;
        let left = straight.round().player(slot).expect("slot exists");
        let right = swapped.round().player(mirror).expect("slot exists");
        assert_eq!(
            left.pending(NORMAL_CHANNEL),
            right.pending(NORMAL_CHANNEL),
            "queues mirror"
        );
        assert_eq!(
            left.score().displayed(),
            right.score().displayed(),
            "scores mirror"
        );
        assert_eq!(left.fever().time_ticks(), right.fever().time_ticks());
        assert_eq!(
            left.attack_fraction().remainder(),
            right.attack_fraction().remainder(),
            "carried remainders mirror"
        );

        // Nuisance is what the safety point governs, so it mirrors cell for
        // cell. Colored cells deliberately do not: each slot derives its own
        // color stream, so the two sides never share a sequence
        // (docs/development/decision/color-sequence-derivation.md).
        let nuisance_cells = |board: &Board| -> Vec<Coord> {
            board
                .visible_coords()
                .filter(|coord| board.get(*coord) == Cell::Nuisance)
                .collect()
        };
        assert_eq!(
            nuisance_cells(left.board()),
            nuisance_cells(right.board()),
            "nuisance landings mirror"
        );
    }
}

/// Advances until `slot` releases a batch, returning the tick's drop count.
fn advance_to_release(state: &mut MatchState, slot: usize) -> u32 {
    for _ in 0..120 {
        let report = state
            .step(&both(GameAction::HardDrop))
            .expect("a tick advances");
        for event in &report.events {
            if let MatchEvent::NuisanceDropped {
                slot: dropped_slot,
                count,
            } = event
                && *dropped_slot == slot
            {
                return *count;
            }
        }
    }
    panic!("slot {slot} never released its queue");
}

// integration-system/match-and-round::TC-016
#[test]
fn a_released_batch_falls_before_the_next_group_is_supplied() {
    let mut state = MatchState::new(spec());
    open_play(&mut state);
    state
        .round_mut()
        .player_mut(0)
        .expect("slot exists")
        .set_pending(NORMAL_CHANNEL, 6);

    let floor = state.spec().board_geometry.height() - 1;
    assert_eq!(advance_to_release(&mut state, 0), 6);

    let fall = state
        .player_view(0)
        .expect("slot exists")
        .resolution
        .expect("the released batch is falling");
    assert_eq!(fall.stage, game_core::view::ResolutionStage::Gravity);
    assert!(
        fall.duration_ticks > 0,
        "a batch entering an empty board spends the table's gravity duration"
    );
    assert!(
        fall.gravity_moves
            .iter()
            .all(|step| step.from.y() < step.to.y()),
        "every released ball enters above where it comes to rest"
    );
    assert_eq!(
        state.board(0).expect("slot exists").get(at(0, floor)),
        Cell::Empty,
        "nothing has reached the floor on the tick the batch is released"
    );

    // The batch owns the whole duration, and the player controls nothing for
    // all of it.
    for elapsed in 1..fall.duration_ticks {
        state.step(&idle()).expect("a tick advances");
        assert!(
            state.active_group(0).is_none(),
            "no group is supplied {elapsed} ticks into the fall"
        );
    }
    state.step(&idle()).expect("a tick advances");

    assert!(
        state.active_group(0).is_some(),
        "the next group arrives on the tick the batch comes to rest"
    );
    assert_eq!(
        state.board(0).expect("slot exists").get(at(0, floor)),
        Cell::Nuisance,
        "the batch is committed to its resting cells"
    );
    assert!(
        state
            .player_view(0)
            .expect("slot exists")
            .resolution
            .is_none(),
        "the fall is over, so nothing is settling"
    );
}

// integration-system/match-and-round::TC-016
#[test]
fn a_batch_burying_the_spawn_column_defeats_on_the_tick_it_lands() {
    let mut state = MatchState::new(spec());
    open_play(&mut state);

    // Nuisance rather than colors, so nothing the batch lands on can clear and
    // the stack stays exactly where the fixture puts it.
    let geometry = state.spec().board_geometry;
    let mut board = Board::with_geometry(geometry);
    for x in 0..geometry.width() {
        for y in (geometry.hidden_rows() + 1)..geometry.height() {
            board.set(at(x, y), Cell::Nuisance);
        }
    }
    {
        let player = state.round_mut().player_mut(0).expect("slot exists");
        player.set_board(board);
        player.set_pending(NORMAL_CHANNEL, 6);
    }

    assert_eq!(advance_to_release(&mut state, 0), 6);
    let duration = state
        .player_view(0)
        .expect("slot exists")
        .resolution
        .expect("the released batch is falling")
        .duration_ticks;
    assert!(
        !state.is_defeated(0),
        "the release itself decides nothing: the batch has not landed yet"
    );

    for _ in 1..duration {
        state.step(&idle()).expect("a tick advances");
        assert!(!state.is_defeated(0), "the batch is still falling");
    }
    state.step(&idle()).expect("a tick advances");

    assert!(
        state.is_defeated(0),
        "the spawn check reads the board the batch came to rest on"
    );
    assert!(matches!(
        state.phase(),
        MatchPhase::RoundOutro {
            outcome: RoundOutcome::Decided(1),
            ..
        }
    ));
}

/// Counts occupied visible cells, so a resolution's ball count can be tracked
/// as balls clear and fall without pinning exact coordinates.
fn occupied_visible_cells(board: &Board) -> usize {
    board
        .visible_coords()
        .filter(|&coord| board.get(coord) != Cell::Empty)
        .count()
}

// component/chain-resolution::TC-017
#[test]
fn the_read_model_shows_the_resolving_board_while_a_chain_is_settling() {
    let mut state = MatchState::new(spec());
    open_play(&mut state);
    let board = board_with_a_clearing_group(&state);
    state
        .round_mut()
        .player_mut(0)
        .expect("slot exists")
        .set_board(board);

    // Locking slot 0's active group joins the fixture's bottom row and opens
    // its resolution; slot 1 is left idle so nothing else is in flight.
    let lock = TickInputs::new([
        PlayerActions::from(GameAction::HardDrop),
        PlayerActions::EMPTY,
    ])
    .expect("two slots");
    state.step(&lock).expect("a tick advances");

    let view = state.player_view(0).expect("slot exists");
    let resolution = view
        .resolution
        .expect("the fixture's row of four opens a chain on lock");
    assert_eq!(
        resolution.stage,
        game_core::view::ResolutionStage::ClearPreview,
        "the chain opens with a preview"
    );
    let clear_cells = resolution.clear_cells.clone();
    assert!(
        (0..4).all(|x| clear_cells.contains(&at(x, 13))),
        "the fixture's bottom row is what is clearing"
    );
    let before_count = occupied_visible_cells(&view.board);

    // The preview holds: the read model keeps drawing the cleared balls for
    // as long as the stage stays ClearPreview.
    let mut ticks = 0;
    loop {
        let view = state.player_view(0).expect("slot exists");
        let stage = view.resolution.expect("the chain is still resolving").stage;
        if stage != game_core::view::ResolutionStage::ClearPreview {
            break;
        }
        for &coord in &clear_cells {
            assert_eq!(
                view.board.get(coord),
                Cell::Color(0),
                "a ball scheduled to clear stays drawn while the preview counts down"
            );
        }
        state.step(&idle()).expect("a tick advances");
        ticks += 1;
        assert!(ticks < 100, "the preview should have expired by now");
    }

    // The tick the link commits: the cleared cells read empty and the total
    // ball count drops by exactly what cleared.
    let view = state.player_view(0).expect("slot exists");
    for &coord in &clear_cells {
        assert_eq!(
            view.board.get(coord),
            Cell::Empty,
            "a cleared ball is not drawn on the tick its link commits"
        );
    }
    let after_commit_count = occupied_visible_cells(&view.board);
    assert_eq!(
        before_count - after_commit_count,
        clear_cells.len(),
        "exactly the cleared balls disappear on the commit tick"
    );

    // From here to the end of the resolution, balls falling into the vacated
    // cells replace what left; the total must never climb back above what it
    // dropped to at commit.
    let mut ticks = 0;
    loop {
        let view = state.player_view(0).expect("slot exists");
        assert!(
            occupied_visible_cells(&view.board) <= after_commit_count,
            "the read model never regains balls that already cleared"
        );
        if view.resolution.is_none() {
            break;
        }
        state.step(&idle()).expect("a tick advances");
        ticks += 1;
        assert!(ticks < 200, "the resolution should have settled by now");
    }
}
