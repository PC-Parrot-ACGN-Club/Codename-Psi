mod common;

use game_core::{
    MatchState,
    input::{GameAction, PlayerActions, TickInputs},
    match_state::{MatchPhase, MatchStepError},
};

fn state() -> MatchState {
    MatchState::new(common::repository_spec(9))
}

#[test]
fn a_locked_group_waits_for_settlement_before_the_next_group_spawns() {
    let mut match_state = state();
    let idle = TickInputs::new([PlayerActions::EMPTY, PlayerActions::EMPTY]).unwrap();
    match_state.step(&idle).unwrap();
    let hard_drop = TickInputs::new([
        PlayerActions::from(GameAction::HardDrop),
        PlayerActions::EMPTY,
    ])
    .unwrap();
    match_state.step(&hard_drop).unwrap();
    assert!(
        match_state.active_group(0).is_none(),
        "settlement owns the field after a lock"
    );
    match_state.step(&idle).unwrap();
    assert!(
        match_state.active_group(0).is_some(),
        "the next group appears only after settlement"
    );
}

#[test]
fn one_tick_requires_exactly_two_slots_and_then_enters_playing() {
    let mut match_state = state();
    let error = match_state
        .step(&TickInputs::EMPTY)
        .expect_err("one slot count is invalid");
    assert_eq!(
        error,
        MatchStepError::ParticipantCount {
            expected: 2,
            actual: 0
        }
    );
    assert_eq!(
        match_state.match_tick(),
        0,
        "rejection must not mutate state"
    );

    let inputs =
        TickInputs::new([PlayerActions::EMPTY, PlayerActions::EMPTY]).expect("two inputs fit");
    let report = match_state
        .step(&inputs)
        .expect("two slots advance the match");
    assert_eq!(report.phase, MatchPhase::Playing);
    assert_eq!(match_state.match_tick(), 1);
}
