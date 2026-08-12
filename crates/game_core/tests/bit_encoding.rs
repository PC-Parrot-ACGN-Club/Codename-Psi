//! Stable bit encoding of `PlayerActions` and slot-occupancy semantics.
//!
//! The encoding is a format, not an implementation detail: checksums,
//! determinism logs and later network payloads are all defined in terms of it,
//! so these tests pin the numeric values rather than round-tripping them.

use game_core::input::{GameAction, PlayerActions, TickInputs};

// docs/test/game-infrastructure.md TC-060
#[test]
fn each_action_occupies_its_documented_bit() {
    let expected = [
        (GameAction::Left, 0b0000_0001),
        (GameAction::Right, 0b0000_0010),
        (GameAction::SoftDrop, 0b0000_0100),
        (GameAction::HardDrop, 0b0000_1000),
        (GameAction::RotateClockwise, 0b0001_0000),
        (GameAction::RotateCounterClockwise, 0b0010_0000),
    ];

    for (action, bits) in expected {
        assert_eq!(
            PlayerActions::from_action(action).bits(),
            bits,
            "{action:?} must stay on its documented bit"
        );
    }
}

// docs/test/game-infrastructure.md TC-060
#[test]
fn reserved_bits_stay_zero_for_every_action_combination() {
    let all = PlayerActions::from_actions([
        GameAction::Left,
        GameAction::Right,
        GameAction::SoftDrop,
        GameAction::HardDrop,
        GameAction::RotateClockwise,
        GameAction::RotateCounterClockwise,
    ]);

    assert_eq!(all.bits(), 0b0011_1111);
    assert_eq!(all.bits() & 0b1100_0000, 0, "bits 6-7 are reserved");
    assert_eq!(PlayerActions::EMPTY.bits(), 0);
}

// docs/test/game-infrastructure.md TC-060
#[test]
fn decoding_rejects_payloads_that_set_reserved_bits() {
    assert_eq!(PlayerActions::from_bits(0b0011_1111), {
        let all = PlayerActions::from_actions([
            GameAction::Left,
            GameAction::Right,
            GameAction::SoftDrop,
            GameAction::HardDrop,
            GameAction::RotateClockwise,
            GameAction::RotateCounterClockwise,
        ]);
        Some(all)
    });

    for reserved in [0b0100_0000, 0b1000_0000, 0b1100_0000] {
        assert_eq!(
            PlayerActions::from_bits(reserved),
            None,
            "a reserved bit must not decode into a valid set"
        );
    }
}

// docs/test/game-infrastructure.md TC-061
#[test]
fn an_absent_participant_is_distinct_from_one_that_did_nothing() {
    let inputs = TickInputs::new([
        PlayerActions::from_action(GameAction::Left),
        PlayerActions::EMPTY,
    ])
    .expect("two participants fit");

    assert_eq!(
        inputs.player(1),
        Some(PlayerActions::EMPTY),
        "slot 1 exists and simply produced no action this tick"
    );
    assert_eq!(
        inputs.player(2),
        None,
        "slot 2 has no participant at all this match"
    );
}
