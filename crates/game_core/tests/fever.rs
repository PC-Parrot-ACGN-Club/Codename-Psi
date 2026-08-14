use game_core::fever::FeverState;
#[test]
fn full_gauge_enters_and_expired_time_exits_at_a_boundary() {
    let mut state = FeverState::new(2, 2, 0, 5);
    state.record_offset(true);
    state.record_offset(true);
    assert!(state.enter_if_full());
    assert!(state.active());
    assert!(!state.tick());
    assert!(state.tick());
    state.exit();
    assert!(!state.active());
}
