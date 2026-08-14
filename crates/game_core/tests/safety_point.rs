use game_core::safety_point::arbitrate_attacks;

#[test]
fn residual_attacks_are_enqueued_only_after_both_offsets_use_the_snapshot() {
    let report = arbitrate_attacks([7, 3], [5, 0]);
    assert_eq!(report.offsets[0].offset, 5);
    assert_eq!(report.offsets[0].sent, 2);
    assert_eq!(report.offsets[1].sent, 3);
    assert_eq!(report.queues_after, [3, 2]);
}
