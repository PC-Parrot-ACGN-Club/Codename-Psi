//! Stable bit encoding of `PlayerActions` and slot-occupancy semantics.
//!
//! The encoding is a format, not an implementation detail: checksums,
//! determinism logs and later network payloads are all defined in terms of it,
//! so these tests pin the numeric values rather than round-tripping them.

use game_core::input::{GameAction, PlayerActions, TickInputs};

// component/game-actions::TC-011
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

// component/game-actions::TC-011
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

// component/game-actions::TC-011
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

// component/game-actions::TC-012
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

// component/game-actions::TC-013
#[test]
fn every_public_decode_entry_rejects_the_reserved_bits() {
    for bits in [0u8, 63] {
        assert_eq!(
            PlayerActions::from_bits(bits).map(PlayerActions::bits),
            Some(bits),
            "{bits} uses only the six defined bits and must decode"
        );
        assert_eq!(
            serde_json::from_str::<PlayerActions>(&bits.to_string())
                .expect("a legal encoding decodes")
                .bits(),
            bits,
            "serde must agree with from_bits on {bits}"
        );
    }

    // Bit 6, bit 7, and both: a corrupt or future-versioned payload.
    for bits in [64u8, 128, 192] {
        assert_eq!(
            PlayerActions::from_bits(bits),
            None,
            "from_bits must reject the reserved bits in {bits}"
        );
        assert!(
            serde_json::from_str::<PlayerActions>(&bits.to_string()).is_err(),
            "serde must not be a way around the reserved-bit invariant ({bits})"
        );
    }
}

// component/game-actions::TC-013
#[test]
fn a_decoded_set_round_trips_through_serialization() {
    let actions = PlayerActions::from_actions([GameAction::Left, GameAction::HardDrop]);
    let encoded = serde_json::to_string(&actions).expect("serializes");

    assert_eq!(encoded, "9", "the wire form is the bare bit value");
    assert_eq!(
        serde_json::from_str::<PlayerActions>(&encoded).expect("decodes"),
        actions
    );
}
