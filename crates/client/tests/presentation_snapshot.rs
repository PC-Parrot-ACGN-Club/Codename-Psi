//! Snapshot projection coverage from `component/presentation-snapshot.md`.

mod presentation_common;

use client::presentation::{
    FEEDBACK_TICKS, FeedbackLine, FeedbackLines, MARKS_PER_CELL, MatchPresentationFrame,
    NUISANCE_ICON_SLOTS, NUISANCE_UNITS, PresentationEffects, PresentationEventConsumer,
    build_snapshot, nuisance_icons, publish_events,
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

/// A localization holding the real shipped catalogs.
fn localization(locale: &str) -> client::i18n::Localization {
    client::i18n::Localization::new(
        locale,
        [
            client::i18n::parse_catalog(include_str!("../../../assets/i18n/en.json"))
                .expect("the shipped English catalog parses"),
            client::i18n::parse_catalog(include_str!("../../../assets/i18n/zh-CN.json"))
                .expect("the shipped Chinese catalog parses"),
        ],
    )
}

// component/presentation-snapshot::TC-008
#[test]
fn one_ticks_facts_leave_one_localized_line_per_participant() {
    let en = localization("en");
    let zh = localization("zh-CN");

    // What reached the opponent outranks what was cancelled.
    let mut lines = FeedbackLines::default();
    lines.observe(&report(
        10,
        vec![
            MatchEvent::AttackArbitrated {
                slot: 0,
                offset: 2,
                sent: 12,
            },
            MatchEvent::AttackArbitrated {
                slot: 1,
                offset: 6,
                sent: 0,
            },
        ],
    ));
    assert_eq!(
        lines.line(0, 10).expect("slot 0 has a line").text(&en),
        "Attack 12"
    );
    assert_eq!(
        lines.line(1, 10).expect("slot 1 has a line").text(&en),
        "Offset 6"
    );
    assert_eq!(
        lines.line(1, 10).expect("slot 1 has a line").text(&zh),
        "抵消 6",
        "the fact's name is localized and its count is the rules' own integer"
    );

    // An all clear outranks the attack the same chain produced.
    let mut clearing = FeedbackLines::default();
    clearing.observe(&report(
        10,
        vec![
            MatchEvent::ChainSettled {
                slot: 0,
                links: 4,
                all_clear: true,
            },
            MatchEvent::AttackArbitrated {
                slot: 0,
                offset: 0,
                sent: 30,
            },
        ],
    ));
    assert_eq!(
        clearing.line(0, 10).expect("slot 0 has a line").text(&zh),
        "全消"
    );
}

// component/presentation-snapshot::TC-008
#[test]
fn a_line_expires_on_its_own_clock_and_an_empty_tick_leaves_it_alone() {
    let mut lines = FeedbackLines::default();
    lines.observe(&report(
        10,
        vec![MatchEvent::AttackArbitrated {
            slot: 0,
            offset: 0,
            sent: 5,
        }],
    ));

    // A tick with nothing to say is not an instruction to clear the screen.
    lines.observe(&report(11, vec![MatchEvent::GroupLocked(0)]));
    assert_eq!(lines.line(0, 11), Some(FeedbackLine::Attack(5)));

    let last = 10 + FEEDBACK_TICKS - 1;
    assert_eq!(lines.line(0, last), Some(FeedbackLine::Attack(5)));
    assert_eq!(lines.line(0, last + 1), None, "the line expires on time");
    assert_eq!(lines.line(1, 10), None, "the other slot said nothing");
}

// component/presentation-snapshot::TC-008
#[test]
fn observing_the_same_tick_twice_shows_the_same_line() {
    let facts = report(
        10,
        vec![MatchEvent::AttackArbitrated {
            slot: 0,
            offset: 3,
            sent: 0,
        }],
    );
    let mut once = FeedbackLines::default();
    once.observe(&facts);
    let mut twice = FeedbackLines::default();
    twice.observe(&facts);
    twice.observe(&facts);

    assert_eq!(once, twice, "the HUD refreshes faster than the rules tick");
}

// component/presentation-snapshot::TC-009
#[test]
fn a_queue_reads_as_tier_icons_heaviest_first() {
    assert!(nuisance_icons(0).is_empty(), "an empty queue shows no icon");
    assert_eq!(nuisance_icons(1), vec![1]);
    assert_eq!(nuisance_icons(5), vec![1, 1, 1, 1, 1]);
    assert_eq!(
        nuisance_icons(6),
        vec![6],
        "a full row is one icon, not six"
    );
    assert_eq!(nuisance_icons(35), vec![30, 1, 1, 1, 1, 1]);
    assert_eq!(
        nuisance_icons(2531),
        vec![1440, 720, 360, 6, 1, 1, 1, 1],
        "every unit is spent before a lighter one is used"
    );

    for count in [0, 1, 29, 30, 179, 1441, 100_000] {
        let icons = nuisance_icons(count);
        assert!(
            icons.len() <= NUISANCE_ICON_SLOTS,
            "{count} spelled out to {} icons",
            icons.len()
        );
        assert!(
            icons.windows(2).all(|pair| pair[0] >= pair[1]),
            "{count} produced icons out of order: {icons:?}"
        );
        assert!(
            icons.iter().sum::<u32>() <= count,
            "{count} produced icons standing for more than the queue holds"
        );
    }

    // The units are exactly the ones the presentation contract names.
    assert_eq!(NUISANCE_UNITS, [1440, 720, 360, 180, 30, 6, 1]);
}

// component/presentation-snapshot::TC-010
#[test]
fn a_loaded_fever_puzzle_marks_its_preset_cells() {
    let state = presentation_common::state(7);
    let spec = state.spec().clone();
    let puzzle = spec
        .fever
        .puzzles
        .puzzles
        .first()
        .expect("the repository book has puzzles")
        .clone();

    let outside_fever = state.view();
    let snapshot = build_snapshot(Some(&outside_fever), None, &spec, AnimationIntensity::Full)
        .expect("a match produces a snapshot");
    assert!(
        snapshot.players[0].preset_cells.is_empty(),
        "a player with no puzzle loaded has no preset to mark"
    );

    let mut in_fever = outside_fever.clone();
    in_fever.players[0].in_fever = true;
    in_fever.players[0].fever_puzzle_id = Some(puzzle.id.clone());
    let snapshot = build_snapshot(Some(&in_fever), None, &spec, AnimationIntensity::Full)
        .expect("a match produces a snapshot");
    let marked: Vec<(u8, u8)> = snapshot.players[0]
        .preset_cells
        .iter()
        .map(|coord| (coord.x(), coord.y()))
        .collect();
    let expected: Vec<(u8, u8)> = puzzle.cells.iter().map(|cell| (cell.x, cell.y)).collect();
    assert_eq!(marked, expected, "the marks are the puzzle's own cells");
    assert!(
        snapshot.players[1].preset_cells.is_empty(),
        "the other side is not in Fever and has nothing marked"
    );

    let mut unknown = outside_fever;
    unknown.players[0].fever_puzzle_id = Some("no-such-puzzle".into());
    let snapshot = build_snapshot(Some(&unknown), None, &spec, AnimationIntensity::Full)
        .expect("a match produces a snapshot");
    assert!(
        snapshot.players[0].preset_cells.is_empty(),
        "a puzzle the book does not hold marks nothing rather than refusing the snapshot"
    );
}

// component/presentation-snapshot::TC-007
#[test]
fn the_density_setting_decides_how_many_marks_a_clear_leaves() {
    let full = PresentationEffects::of(AnimationIntensity::Full);
    let reduced = PresentationEffects::of(AnimationIntensity::Reduced);

    assert_eq!(full.marks_per_cell(), MARKS_PER_CELL);
    assert_eq!(
        reduced.marks_per_cell(),
        0,
        "reduced spends nothing per cell; the caller keeps one hint for the whole fact"
    );
    assert!(
        full.particle_density > reduced.particle_density,
        "the two settings differ in the parameter the marks are spent from"
    );
}
