//! Determinism coverage from `integration-system/determinism-and-snapshot.md`.

mod common;

use game_core::{
    MatchState,
    determinism::{MatchRng, REGISTERED_STREAMS, StateChecksum, StreamName},
    digest::DigestWriter,
    input::{GameAction, PlayerActions, TickInputs},
    match_spec::LockedMatchSpec,
    match_state::MatchPhase,
    player::{FEVER_CHANNEL, NORMAL_CHANNEL},
    snapshot::{
        SNAPSHOT_SCHEMA_VERSION, SnapshotError, VERIFICATION_LOG_FORMAT_VERSION, VerificationError,
        record_verification_log, run_verification_log,
    },
};

fn spec() -> LockedMatchSpec {
    common::repository_spec(0x1)
}

/// FNV over a stream's first 32 outputs, which pins the whole sequence.
fn stream_digest(stream: StreamName, slot: u8) -> u64 {
    let mut rng = MatchRng::derive(0x1, 0, 0, slot, stream);
    let mut writer = DigestWriter::new();
    for _ in 0..32 {
        writer.u32(rng.next_u32());
    }
    writer.finish().0
}

/// A repeatable input log with movement, rotation and drops.
fn input_log(ticks: usize) -> Vec<[PlayerActions; 2]> {
    (0..ticks)
        .map(|tick| {
            let a = match tick % 7 {
                0 => PlayerActions::from(GameAction::Left),
                1 => PlayerActions::from(GameAction::RotateClockwise),
                2 => PlayerActions::from(GameAction::Right),
                3 => PlayerActions::from(GameAction::SoftDrop),
                4 => PlayerActions::from(GameAction::RotateCounterClockwise),
                5 => PlayerActions::EMPTY,
                _ => PlayerActions::from(GameAction::HardDrop),
            };
            let b = match tick % 5 {
                0 => PlayerActions::from(GameAction::Right),
                1 => PlayerActions::from(GameAction::SoftDrop),
                2 => PlayerActions::from(GameAction::HardDrop),
                _ => PlayerActions::EMPTY,
            };
            [a, b]
        })
        .collect()
}

fn run(state: &mut MatchState, log: &[[PlayerActions; 2]]) {
    for actions in log {
        let inputs = TickInputs::new(*actions).expect("two slots");
        state.step(&inputs).expect("a tick advances");
    }
}

// integration-system/determinism-and-snapshot::TC-001
#[test]
fn the_named_streams_are_pinned_by_golden_vectors() {
    // Changing any of these means the random algorithm version must move too.
    let goldens = [
        (StreamName::Color, 0x11a1_aa8d_7bd2_a8ee_u64),
        (StreamName::Nuisance, 0x5ad4_2123_1fd5_a07c),
        (StreamName::FeverPuzzle, 0xef7c_1353_4836_45c1),
    ];
    for (stream, expected) in goldens {
        assert_eq!(
            stream_digest(stream, 0),
            expected,
            "{} drifted; bump RNG_ALGORITHM_VERSION if that was intended",
            stream.as_str()
        );
        // The same key derives the same sequence every time.
        assert_eq!(stream_digest(stream, 0), stream_digest(stream, 0));
    }
    assert_eq!(REGISTERED_STREAMS.len(), 3);
}

// integration-system/determinism-and-snapshot::TC-002
#[test]
fn an_unregistered_stream_name_cannot_be_derived() {
    let error = StreamName::from_name("unknown-stream").expect_err("not a registered stream");
    assert_eq!(error.0, "unknown-stream");

    for stream in REGISTERED_STREAMS {
        assert_eq!(
            StreamName::from_name(stream.as_str()),
            Ok(stream),
            "the registered names still resolve"
        );
    }
}

// integration-system/determinism-and-snapshot::TC-003
#[test]
fn the_two_participants_streams_are_independent() {
    assert_ne!(
        stream_digest(StreamName::Color, 0),
        stream_digest(StreamName::Color, 1),
        "the derivation key includes the participant slot"
    );

    // Advancing one slot's stream does not move the other's.
    let mut slot0 = MatchRng::derive(0x1, 0, 0, 0, StreamName::Color);
    let mut slot1 = MatchRng::derive(0x1, 0, 0, 1, StreamName::Color);
    let expected = {
        let mut fresh = MatchRng::derive(0x1, 0, 0, 1, StreamName::Color);
        fresh.next_u32()
    };
    for _ in 0..100 {
        slot0.next_u32();
    }
    assert_eq!(slot1.next_u32(), expected);
}

// integration-system/determinism-and-snapshot::TC-004
#[test]
fn the_same_log_replays_to_the_same_checkpoints() {
    let library = common::repository_library();
    let profile_id = game_core::config::RuleProfileId(common::PROFILE_ID.into());
    let log = record_verification_log(&spec(), input_log(1_000), 100);
    assert_eq!(log.format_version, VERIFICATION_LOG_FORMAT_VERSION);
    assert_eq!(log.checkpoints.len(), 10, "one checkpoint every 100 ticks");

    let first = run_verification_log(&log, &library, &profile_id).expect("the log runs");
    let second = run_verification_log(&log, &library, &profile_id).expect("the log runs again");

    assert!(first.is_consistent(), "{:?}", first.differences);
    assert!(second.is_consistent());
    assert_eq!(first.checkpoints, second.checkpoints);
}

// integration-system/determinism-and-snapshot::TC-005
#[test]
fn the_same_log_replays_identically_in_a_separate_process() {
    // Re-runs this binary's checksum-printing case as its own process, so the
    // comparison really does cross process boundaries rather than only threads.
    let exe = std::env::current_exe().expect("the test binary knows its own path");
    let output = std::process::Command::new(exe)
        .args([
            "--exact",
            "prints_the_final_checksum_for_the_cross_process_check",
            "--ignored",
            "--nocapture",
        ])
        .output()
        .expect("the test binary can run itself");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let printed = stdout
        .lines()
        .find_map(|line| line.strip_prefix("CHECKSUM "))
        .expect("the child prints its checksum");

    assert_eq!(
        printed,
        final_checksum().0.to_string(),
        "a separate process must agree; nothing may depend on hash order or addresses"
    );
}

/// Shared body of the cross-process check.
fn final_checksum() -> StateChecksum {
    let mut state = MatchState::new(spec());
    run(&mut state, &input_log(500));
    state.checksum()
}

#[test]
#[ignore = "invoked as a child process by the cross-process check"]
fn prints_the_final_checksum_for_the_cross_process_check() {
    println!("CHECKSUM {}", final_checksum().0);
}

// integration-system/determinism-and-snapshot::TC-006
#[test]
fn a_thousand_ticks_with_rollbacks_match_a_straight_run() {
    let log = input_log(1_000);

    let mut straight = MatchState::new(spec());
    run(&mut straight, &log);

    let mut rolled = MatchState::new(spec());
    let mut tick = 0;
    while tick < log.len() {
        let chunk = (tick + 50).min(log.len());
        let checkpoint = rolled.snapshot();
        run(&mut rolled, &log[tick..chunk]);
        // Roll back the last ten ticks and replay them.
        let rewind = chunk.saturating_sub(10).max(tick);
        rolled = checkpoint
            .restore(&spec())
            .expect("a snapshot of this build restores");
        run(&mut rolled, &log[tick..rewind]);
        run(&mut rolled, &log[rewind..chunk]);
        tick = chunk;
    }

    assert_eq!(
        straight.checksum(),
        rolled.checksum(),
        "rolling back and replaying must not accumulate drift"
    );
    assert_eq!(straight.match_tick(), rolled.match_tick());
}

// integration-system/determinism-and-snapshot::TC-007
#[test]
fn restoring_from_five_different_phases_converges_with_the_baseline() {
    let log = input_log(600);
    // Snapshot points chosen to land in different kinds of state.
    for snapshot_at in [5_usize, 190, 220, 300, 450] {
        let mut baseline = MatchState::new(spec());
        run(&mut baseline, &log);

        let mut forked = MatchState::new(spec());
        run(&mut forked, &log[..snapshot_at]);
        let snapshot = forked.snapshot();
        let mut restored = snapshot.restore(&spec()).expect("the snapshot restores");
        assert_eq!(
            restored.checksum(),
            forked.checksum(),
            "a restore is an exact copy at snapshot point {snapshot_at}"
        );
        run(&mut restored, &log[snapshot_at..]);

        assert_eq!(
            baseline.checksum(),
            restored.checksum(),
            "restoring at tick {snapshot_at} must converge with the baseline"
        );
        for slot in 0..2 {
            let a = baseline.round().player(slot).expect("slot exists");
            let b = restored.round().player(slot).expect("slot exists");
            assert_eq!(a.pending(NORMAL_CHANNEL), b.pending(NORMAL_CHANNEL));
            assert_eq!(a.pending(FEVER_CHANNEL), b.pending(FEVER_CHANNEL));
            assert_eq!(a.drop_state(NORMAL_CHANNEL), b.drop_state(NORMAL_CHANNEL));
            assert_eq!(a.fever().time_ticks(), b.fever().time_ticks());
            assert_eq!(a.bags(), b.bags());
            assert_eq!(a.stream().cursor(), b.stream().cursor());
            assert_eq!(a.stream().swaps_l_and_j(), b.stream().swaps_l_and_j());
        }
    }
}

// integration-system/determinism-and-snapshot::TC-008
#[test]
fn a_snapshot_forks_under_different_inputs_and_only_rejoins_on_identical_ones() {
    let mut origin = MatchState::new(spec());
    run(&mut origin, &input_log(200));

    // Lateral input only forks the state while somebody is holding a group:
    // during a resolve the rules ignore it by design. Advance to the first tick
    // where both players control one, so what forks the state is the input
    // rather than where 200 ticks happened to land in the resolve pacing.
    for _ in 0..600 {
        if origin.active_group(0).is_some() && origin.active_group(1).is_some() {
            break;
        }
        run(&mut origin, &[[PlayerActions::EMPTY; 2]]);
    }
    assert!(
        origin.active_group(0).is_some() && origin.active_group(1).is_some(),
        "neither player ever got a group back to steer"
    );
    let snapshot = origin.snapshot();

    let diverge_a: Vec<_> = (0..20)
        .map(|_| [PlayerActions::from(GameAction::Left); 2])
        .collect();
    let diverge_b: Vec<_> = (0..20)
        .map(|_| [PlayerActions::from(GameAction::Right); 2])
        .collect();
    let shared = input_log(100);

    let mut left = snapshot.clone().restore(&spec()).expect("restores");
    let mut right = snapshot.clone().restore(&spec()).expect("restores");
    run(&mut left, &diverge_a);
    run(&mut right, &diverge_b);
    assert_ne!(left.checksum(), right.checksum(), "different inputs fork");

    run(&mut left, &shared);
    run(&mut right, &shared);
    assert_ne!(
        left.checksum(),
        right.checksum(),
        "a fork does not heal itself once the states have separated"
    );

    // Feeding the identical 120 ticks from the same snapshot does converge.
    let mut rerun = snapshot.restore(&spec()).expect("restores");
    run(&mut rerun, &diverge_a);
    run(&mut rerun, &shared);
    assert_eq!(rerun.checksum(), left.checksum());
}

// integration-system/determinism-and-snapshot::TC-009
#[test]
fn every_persistent_field_in_the_coverage_table_moves_the_checksum() {
    let mut base = MatchState::new(spec());
    run(&mut base, &input_log(240));
    let baseline = base.checksum();

    /// One named edit to a single persistent field.
    type Edit = (&'static str, Box<dyn Fn(&mut MatchState)>);

    // Each edit touches one domain of the snapshot coverage table.
    let edits: Vec<Edit> = vec![
        (
            "queue and column order",
            Box::new(|state: &mut MatchState| {
                let player = state.round_mut().player_mut(0).expect("slot exists");
                player.set_pending(NORMAL_CHANNEL, player.pending(NORMAL_CHANNEL) + 1);
            }),
        ),
        (
            "board",
            Box::new(|state: &mut MatchState| {
                let mut board = state.board(1).expect("slot exists").clone();
                let coord = board.coord(0, 13).expect("in range");
                board.set(coord, game_core::board::Cell::Nuisance);
                state
                    .round_mut()
                    .player_mut(1)
                    .expect("slot exists")
                    .set_board(board);
            }),
        ),
        (
            "player-level Fever time",
            Box::new(|state: &mut MatchState| {
                state
                    .round_mut()
                    .player_mut(0)
                    .expect("slot exists")
                    .fever_mut()
                    .reward_time(60);
            }),
        ),
    ];

    for (name, edit) in edits {
        let mut edited = base.clone();
        edit(&mut edited);
        assert_ne!(
            edited.checksum(),
            baseline,
            "changing the {name} must move the checksum"
        );
        // Restoring the untouched copy returns the baseline value.
        assert_eq!(base.checksum(), baseline);
    }

    // The random stream position is persistent state too.
    let mut advanced = base.clone();
    run(&mut advanced, &input_log(1));
    assert_ne!(advanced.checksum(), baseline);
}

// integration-system/determinism-and-snapshot::TC-010
#[test]
fn reading_events_does_not_change_the_checksum() {
    let log = input_log(200);
    let mut consumed = MatchState::new(spec());
    let mut ignored = MatchState::new(spec());

    let mut seen = 0_usize;
    for actions in &log {
        let inputs = TickInputs::new(*actions).expect("two slots");
        let report = consumed.step(&inputs).expect("a tick advances");
        // One side reads every event; the other never looks.
        seen += report.events.len();
        let _ = ignored.step(&inputs).expect("a tick advances");
    }
    assert!(seen > 0, "the run really did produce events");
    assert_eq!(
        consumed.checksum(),
        ignored.checksum(),
        "the event consumption cursor is not rules state"
    );
}

// integration-system/determinism-and-snapshot::TC-011
#[test]
fn a_mismatched_header_refuses_to_restore() {
    let mut state = MatchState::new(spec());
    run(&mut state, &input_log(50));
    let good = state.snapshot();
    assert!(
        good.clone().restore(&spec()).is_ok(),
        "the untouched snapshot restores"
    );

    let mut wrong_schema = good.clone();
    wrong_schema.snapshot_schema_version += 1;
    assert!(matches!(
        wrong_schema.restore(&spec()),
        Err(SnapshotError::UnsupportedSchema { .. })
    ));

    let mut wrong_digest = good.clone();
    wrong_digest.digests.root = game_core::digest::ContentDigest(0xdead_beef);
    assert!(matches!(
        wrong_digest.restore(&spec()),
        Err(SnapshotError::DigestMismatch { .. })
    ));

    for bump in [
        (|snapshot: &mut game_core::snapshot::MatchSnapshot| snapshot.algorithms.rng += 1)
            as fn(&mut _),
        |snapshot: &mut game_core::snapshot::MatchSnapshot| snapshot.algorithms.state_codec += 1,
        |snapshot: &mut game_core::snapshot::MatchSnapshot| snapshot.algorithms.digest += 1,
    ] {
        let mut wrong = good.clone();
        bump(&mut wrong);
        assert!(matches!(
            wrong.restore(&spec()),
            Err(SnapshotError::AlgorithmMismatch { .. })
        ));
    }

    assert_eq!(good.snapshot_schema_version, SNAPSHOT_SCHEMA_VERSION);
}

// integration-system/determinism-and-snapshot::TC-012
#[test]
fn snapshotting_at_any_settlement_tick_keeps_the_read_models_consistent() {
    let log = input_log(400);
    let mut baseline = MatchState::new(spec());
    run(&mut baseline, &log);

    // Every tick of a window that spans settlement and control is a valid
    // snapshot point; none of them may fork.
    for snapshot_at in 180..240 {
        let mut forked = MatchState::new(spec());
        run(&mut forked, &log[..snapshot_at]);
        let mut restored = forked
            .snapshot()
            .restore(&spec())
            .expect("the snapshot restores");
        run(&mut restored, &log[snapshot_at..]);
        assert_eq!(
            baseline.checksum(),
            restored.checksum(),
            "snapshotting at tick {snapshot_at} forked the run"
        );
        assert_eq!(
            matches!(baseline.phase(), MatchPhase::Playing),
            matches!(restored.phase(), MatchPhase::Playing)
        );
    }
}

// integration-system/determinism-and-snapshot::TC-011
#[test]
fn a_log_recorded_against_other_content_is_refused() {
    let library = common::repository_library();
    let profile_id = game_core::config::RuleProfileId(common::PROFILE_ID.into());
    let mut log = record_verification_log(&spec(), input_log(20), 10);

    log.format_version += 1;
    assert!(matches!(
        run_verification_log(&log, &library, &profile_id),
        Err(VerificationError::UnsupportedFormat { .. })
    ));

    log.format_version = VERIFICATION_LOG_FORMAT_VERSION;
    log.digests.root = game_core::digest::ContentDigest(0xfeed_face);
    assert!(matches!(
        run_verification_log(&log, &library, &profile_id),
        Err(VerificationError::DigestMismatch { .. })
    ));
}
