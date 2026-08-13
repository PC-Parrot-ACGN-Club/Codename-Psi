//! Source-independent conflict rules applied before rules consume `PlayerActions`.

use game_core::input::{GameAction, PlayerActions};

fn raw(actions: impl IntoIterator<Item = GameAction>) -> PlayerActions {
    PlayerActions::from_actions(actions)
}

/// Normalization must be stable: re-applying it never changes the result again.
fn normalize_once_and_check_idempotence(input: PlayerActions) -> PlayerActions {
    let normalized = input.normalized();
    assert_eq!(
        normalized.normalized(),
        normalized,
        "normalizing an already canonical value must be a no-op"
    );
    normalized
}

// component/game-actions::TC-005
#[test]
fn simultaneous_left_and_right_normalize_to_no_horizontal_direction() {
    let normalized =
        normalize_once_and_check_idempotence(raw([GameAction::Left, GameAction::Right]));

    assert!(!normalized.contains(GameAction::Left));
    assert!(!normalized.contains(GameAction::Right));
    assert!(normalized.is_empty());
}

// component/game-actions::TC-006
#[test]
fn simultaneous_rotations_normalize_to_no_rotation() {
    let normalized = normalize_once_and_check_idempotence(raw([
        GameAction::RotateClockwise,
        GameAction::RotateCounterClockwise,
    ]));

    assert!(!normalized.contains(GameAction::RotateClockwise));
    assert!(!normalized.contains(GameAction::RotateCounterClockwise));
    assert!(normalized.is_empty());
}

// component/game-actions::TC-007
#[test]
fn hard_drop_wins_over_soft_drop() {
    let normalized =
        normalize_once_and_check_idempotence(raw([GameAction::SoftDrop, GameAction::HardDrop]));

    assert!(normalized.contains(GameAction::HardDrop));
    assert!(!normalized.contains(GameAction::SoftDrop));
}

/// component/game-actions::TC-008 — one case per single-action row.
macro_rules! single_action_is_preserved {
    ($($name:ident => $action:expr),+ $(,)?) => {
        $(
            // component/game-actions::TC-008
            #[test]
            fn $name() {
                let input = raw([$action]);

                let normalized = normalize_once_and_check_idempotence(input);

                assert_eq!(
                    normalized, input,
                    "a lone action has no conflict partner and must survive"
                );
            }
        )+
    };
}

single_action_is_preserved! {
    lone_left_is_preserved => GameAction::Left,
    lone_right_is_preserved => GameAction::Right,
    lone_soft_drop_is_preserved => GameAction::SoftDrop,
    lone_hard_drop_is_preserved => GameAction::HardDrop,
    lone_rotate_clockwise_is_preserved => GameAction::RotateClockwise,
    lone_rotate_counter_clockwise_is_preserved => GameAction::RotateCounterClockwise,
}

// component/game-actions::TC-008
#[test]
fn conflict_free_combination_is_preserved() {
    let input = raw([
        GameAction::Left,
        GameAction::SoftDrop,
        GameAction::RotateClockwise,
    ]);

    let normalized = normalize_once_and_check_idempotence(input);

    assert_eq!(normalized, input);
}

// component/game-actions::TC-008
#[test]
fn unrelated_action_survives_a_conflicting_pair() {
    let input = raw([GameAction::Left, GameAction::Right, GameAction::SoftDrop]);

    let normalized = normalize_once_and_check_idempotence(input);

    assert_eq!(
        normalized,
        raw([GameAction::SoftDrop]),
        "only the horizontal pair is resolved; the unrelated action is untouched"
    );
}

// component/game-actions::TC-008
#[test]
fn all_three_conflicts_resolve_independently_in_one_tick() {
    let input = raw([
        GameAction::Left,
        GameAction::Right,
        GameAction::RotateClockwise,
        GameAction::RotateCounterClockwise,
        GameAction::SoftDrop,
        GameAction::HardDrop,
    ]);

    let normalized = normalize_once_and_check_idempotence(input);

    assert_eq!(
        normalized,
        raw([GameAction::HardDrop]),
        "horizontal and rotation pairs clear; the drop conflict keeps hard drop"
    );
}
