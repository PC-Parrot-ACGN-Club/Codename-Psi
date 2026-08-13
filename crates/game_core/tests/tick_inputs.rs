//! Participant slot capacity, ordering, and value semantics of `TickInputs`.

use game_core::input::{GameAction, MAX_PLAYERS, PlayerActions, TickInputs, TickInputsError};

fn actions(actions: impl IntoIterator<Item = GameAction>) -> PlayerActions {
    PlayerActions::from_actions(actions)
}

/// Eight distinguishable action sets, one per participant slot.
fn distinct_slot_actions() -> [PlayerActions; MAX_PLAYERS] {
    [
        actions([GameAction::Left]),
        actions([GameAction::Right]),
        actions([GameAction::SoftDrop]),
        actions([GameAction::HardDrop]),
        actions([GameAction::RotateClockwise]),
        actions([GameAction::RotateCounterClockwise]),
        actions([GameAction::Left, GameAction::SoftDrop]),
        actions([GameAction::Right, GameAction::HardDrop]),
    ]
}

// component/game-actions::TC-001
#[test]
fn zero_participants_construct_empty_tick_inputs() {
    let inputs = TickInputs::new(Vec::<PlayerActions>::new()).expect("zero participants fit");

    assert_eq!(inputs.len(), 0);
    assert!(inputs.is_empty());
    assert_eq!(inputs.active(), &[] as &[PlayerActions]);
    for slot in 0..MAX_PLAYERS {
        assert_eq!(
            inputs.player(slot),
            None,
            "slot {slot} must report no participant"
        );
    }
}

// component/game-actions::TC-002
#[test]
fn two_participants_keep_slot_order_and_clear_the_tail() {
    let slot0 = actions([GameAction::Left]);
    let slot1 = actions([GameAction::HardDrop]);

    let inputs = TickInputs::new([slot0, slot1]).expect("two participants fit");

    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs.player(0), Some(slot0));
    assert_eq!(inputs.player(1), Some(slot1));
    for slot in 2..MAX_PLAYERS {
        assert_eq!(
            inputs.player(slot),
            None,
            "slot {slot} is past the participant count"
        );
    }
}

// component/game-actions::TC-003
#[test]
fn eight_participants_reach_capacity_without_error() {
    let expected = distinct_slot_actions();

    let inputs = TickInputs::new(expected).expect("eight participants fit");

    assert_eq!(inputs.len(), MAX_PLAYERS);
    assert_eq!(inputs.active(), expected.as_slice());
    for (slot, expected_actions) in expected.iter().enumerate() {
        assert_eq!(
            inputs.player(slot),
            Some(*expected_actions),
            "slot {slot} must keep its own value"
        );
    }
}

// component/game-actions::TC-004
#[test]
fn nine_participants_are_rejected_without_truncation() {
    let mut too_many = distinct_slot_actions().to_vec();
    too_many.push(actions([GameAction::SoftDrop, GameAction::RotateClockwise]));
    assert_eq!(too_many.len(), MAX_PLAYERS + 1);

    let error = TickInputs::new(&too_many).expect_err("nine participants exceed capacity");

    assert_eq!(
        error,
        TickInputsError::TooManyPlayers {
            found: MAX_PLAYERS + 1,
            maximum: MAX_PLAYERS,
        },
        "the error must report the rejected count instead of silently truncating"
    );
}

// component/game-actions::TC-009
#[test]
fn repeating_an_action_across_ticks_yields_the_same_logical_value() {
    let held = actions([GameAction::SoftDrop]);

    let ticks: Vec<TickInputs> = (100..103)
        .map(|_| TickInputs::new([held]).expect("one participant fits"))
        .collect();

    for (offset, tick) in ticks.iter().enumerate() {
        let tick_index = 100 + offset;
        assert!(
            tick.player(0)
                .expect("slot 0 is active")
                .contains(GameAction::SoftDrop),
            "tick {tick_index} must express the held action"
        );
    }
    assert_eq!(
        ticks[0], ticks[1],
        "no held/edge state may differentiate equal logical inputs"
    );
    assert_eq!(ticks[1], ticks[2]);
}

// component/game-actions::TC-010
#[test]
fn player_actions_support_copy_and_equality() {
    let original = actions([GameAction::Left, GameAction::SoftDrop]);
    let copy = original;

    assert_eq!(copy, original);

    let first = TickInputs::new([original]).expect("one participant fits");
    let second = TickInputs::new([copy]).expect("one participant fits");

    assert_eq!(
        original, copy,
        "constructing tick inputs must not mutate either value"
    );
    assert_eq!(first.player(0), Some(original));
    assert_eq!(second.player(0), Some(copy));
    assert_eq!(first, second);
}
