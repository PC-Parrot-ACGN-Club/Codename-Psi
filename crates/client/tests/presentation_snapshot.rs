//! Snapshot projection coverage from `component/presentation-snapshot.md`.

mod presentation_common;

use client::presentation::{
    MatchPresentationFrame, PresentationEventConsumer, build_snapshot, publish_events,
};
use client::settings::AnimationIntensity;
use game_core::{
    board::{Cell, Coord},
    match_state::{MatchEvent, MatchPhase, MatchStepReport},
    view::{ResolutionStage, ResolutionView},
};

fn report(tick: u64, events: Vec<MatchEvent>) -> MatchStepReport {
    MatchStepReport {
        match_tick: tick,
        phase: MatchPhase::Playing,
        events,
    }
}

// component/presentation-snapshot::TC-001
#[test]
fn snapshot_is_complete_in_normal_fever_and_resolution_phases() {
    let state = presentation_common::state(7);
    let spec = state.spec().clone();
    let normal = state.view();
    let mut fever = normal.clone();
    fever.players[0].in_fever = true;
    fever.players[0].fever_target = Some(6);
    let mut resolving = normal.clone();
    resolving.players[0].resolution = Some(ResolutionView {
        stage: ResolutionStage::ClearPreview,
        chain_index: 2,
        elapsed_ticks: 3,
        duration_ticks: 12,
        clear_cells: vec![],
        gravity_moves: vec![],
    });

    for view in [normal, fever, resolving] {
        let snapshot = build_snapshot(Some(&view), None, &spec, AnimationIntensity::Full)
            .expect("a match produces a snapshot");
        assert_eq!(snapshot.players.len(), 2);
        for player in &snapshot.players {
            assert_eq!(player.next_drops.len(), 3);
            assert!(!player.drop_set_id.0.is_empty());
            let _ = (&player.board, player.active_drop, player.score);
            let _ = (player.pending_garbage, player.fever_garbage);
            let _ = (
                player.fever_gauge,
                player.fever_time_ticks,
                player.fever_target,
            );
            let _ = (player.chain_count, player.overflow_risk, &player.resolution);
        }
        let _ = (
            snapshot.round,
            snapshot.wins,
            snapshot.phase,
            snapshot.result,
        );
    }
}

// component/presentation-snapshot::TC-002
#[test]
fn no_match_instance_produces_no_snapshot_or_diagnostic() {
    let spec = presentation_common::spec(7);
    assert_eq!(
        build_snapshot(None, None, &spec, AnimationIntensity::Full),
        None
    );
}

// component/presentation-snapshot::TC-003
#[test]
fn events_keep_report_order_and_receive_unique_zero_based_ordinals() {
    let facts = [
        MatchEvent::GroupLocked(0),
        MatchEvent::GroupLocked(1),
        MatchEvent::FeverEntered(0),
        MatchEvent::NuisanceDropped { slot: 0, count: 6 },
        MatchEvent::NuisanceDropped { slot: 1, count: 3 },
    ];
    for count in [0, 1, 5] {
        let events = publish_events(
            &report(42, facts[..count].to_vec()),
            AnimationIntensity::Full,
        );
        assert_eq!(events.len(), count);
        for (ordinal, event) in events.iter().enumerate() {
            assert_eq!(
                (event.id.match_tick, event.id.ordinal),
                (42, ordinal as u16)
            );
            assert_eq!(event.fact, facts[ordinal]);
        }
    }
}

// component/presentation-snapshot::TC-004
#[test]
fn duplicate_event_ids_are_performed_only_once() {
    let events = publish_events(
        &report(
            8,
            vec![
                MatchEvent::GroupLocked(0),
                MatchEvent::FeverEntered(0),
                MatchEvent::NuisanceDropped { slot: 1, count: 3 },
            ],
        ),
        AnimationIntensity::Full,
    );
    let mut consumer = PresentationEventConsumer::default();
    for _ in 0..2 {
        for event in &events {
            consumer.consume(event);
        }
    }
    assert_eq!(consumer.performed_count(), 3);
}

// component/presentation-snapshot::TC-005
#[test]
fn discarding_every_event_does_not_remove_resident_match_facts() {
    let mut state = presentation_common::state(9);
    for _ in 0..30 {
        state.step(&presentation_common::idle()).expect("tick");
    }
    let view = state.view();
    let snapshot = build_snapshot(Some(&view), None, state.spec(), AnimationIntensity::Full)
        .expect("snapshot");
    let rebuilt = MatchPresentationFrame::from_snapshot(&snapshot);

    assert_eq!(rebuilt.snapshot, snapshot);
    assert_eq!(rebuilt.snapshot.players[0].board, view.players[0].board);
    assert_eq!(rebuilt.snapshot.players[0].score, view.players[0].score);
    assert_eq!(rebuilt.snapshot.wins, view.wins);
}

// component/presentation-snapshot::TC-006
#[test]
fn momentum_uses_attack_pressure_overflow_and_fever_facts_only() {
    let state = presentation_common::state(11);
    let mut view = state.view();
    view.players[1].pending[0] = 30;
    let attacked = report(
        1,
        vec![MatchEvent::AttackArbitrated {
            slot: 0,
            offset: 0,
            sent: 12,
        }],
    );
    let snapshot = build_snapshot(
        Some(&view),
        Some(&attacked),
        state.spec(),
        AnimationIntensity::Full,
    )
    .expect("snapshot");
    assert_eq!(snapshot.momentum.advantage_side, Some(0));

    let spawn = view.players[0].board.geometry().spawn_column();
    let y = view.players[0].board.geometry().hidden_rows();
    view.players[0]
        .board
        .set(Coord::new(spawn, y).expect("coord"), Cell::Color(0));
    view.players[0].pending[0] = 60;
    view.players[1].pending = [0, 0];
    let snapshot = build_snapshot(Some(&view), None, state.spec(), AnimationIntensity::Full)
        .expect("snapshot");
    assert_eq!(snapshot.momentum.advantage_side, Some(1));

    let mut symmetric = state.view();
    symmetric.players[0].pending = [10, 0];
    symmetric.players[1].pending = [10, 0];
    assert_eq!(
        build_snapshot(
            Some(&symmetric),
            None,
            state.spec(),
            AnimationIntensity::Full
        )
        .expect("snapshot")
        .momentum
        .advantage_side,
        None
    );

    symmetric.players[0].in_fever = true;
    assert_eq!(
        build_snapshot(
            Some(&symmetric),
            None,
            state.spec(),
            AnimationIntensity::Reduced
        )
        .expect("snapshot")
        .momentum
        .advantage_side,
        Some(0)
    );
}

// component/presentation-snapshot::TC-007
#[test]
fn animation_intensity_changes_only_disposable_effect_parameters() {
    let state = presentation_common::state(13);
    let view = state.view();
    let report = report(
        3,
        vec![MatchEvent::GroupLocked(0), MatchEvent::FeverEntered(1)],
    );
    let full_snapshot = build_snapshot(
        Some(&view),
        Some(&report),
        state.spec(),
        AnimationIntensity::Full,
    )
    .expect("snapshot");
    let reduced_snapshot = build_snapshot(
        Some(&view),
        Some(&report),
        state.spec(),
        AnimationIntensity::Reduced,
    )
    .expect("snapshot");
    // The effect parameters are the one field the setting is allowed to move:
    // the resident layer reads them off the snapshot so that rebuilding the
    // screen from a single snapshot stays sufficient. Normalising just that
    // field and comparing the rest proves no rule fact rode along with it.
    assert_ne!(full_snapshot.effects, reduced_snapshot.effects);
    let mut rule_fields = reduced_snapshot.clone();
    rule_fields.effects = full_snapshot.effects;
    assert_eq!(full_snapshot, rule_fields);

    let full = publish_events(&report, AnimationIntensity::Full);
    let reduced = publish_events(&report, AnimationIntensity::Reduced);
    assert_eq!(
        full.iter()
            .map(|event| (event.id, &event.fact))
            .collect::<Vec<_>>(),
        reduced
            .iter()
            .map(|event| (event.id, &event.fact))
            .collect::<Vec<_>>()
    );
    assert_ne!(full[0].effects, reduced[0].effects);
}
