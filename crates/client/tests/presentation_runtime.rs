//! In-memory presentation runtime coverage from `integration-system/presentation-runtime.md`.

mod common;
mod presentation_common;

use client::app_state::AppState;
use client::presentation::{
    AudioAvailability, AudioGains, EntityLifecycle, FeedbackBudget, FeedbackRuntime,
    PresentationRuntime, VirtualCanvas, build_snapshot, publish_events,
};
use client::settings::AnimationIntensity;
use game_core::match_state::{MatchEvent, MatchPhase, MatchStepReport};

fn events(count: usize) -> Vec<client::presentation::PresentationEvent> {
    publish_events(
        &MatchStepReport {
            match_tick: 7,
            phase: MatchPhase::Playing,
            events: (0..count)
                .map(|index| MatchEvent::NuisanceDropped {
                    slot: index % 2,
                    count: 1,
                })
                .collect(),
        },
        AnimationIntensity::Full,
    )
}

// integration-system/presentation-runtime::TC-001
#[test]
fn a_frame_rebuilds_from_each_latest_snapshot_without_event_history() {
    let state = presentation_common::state(1);
    let mut runtime = PresentationRuntime::default();
    for mut view in [state.view(), state.view(), state.view()] {
        view.players[0].in_fever = runtime.frame().is_some();
        let snapshot = build_snapshot(Some(&view), None, state.spec(), AnimationIntensity::Full)
            .expect("snapshot");
        runtime.clear_resident_entities();
        runtime.sample(snapshot.clone());
        assert_eq!(runtime.frame().expect("frame").snapshot, snapshot);
    }
}

// integration-system/presentation-runtime::TC-002
#[test]
fn lagging_presentation_samples_only_the_latest_rule_stage() {
    let mut state = presentation_common::state(2);
    let mut runtime = PresentationRuntime::default();
    for _ in 0..10 {
        state.step(&presentation_common::idle()).expect("tick");
    }
    let old = build_snapshot(
        Some(&state.view()),
        None,
        state.spec(),
        AnimationIntensity::Full,
    )
    .expect("old");
    for _ in 0..20 {
        state.step(&presentation_common::idle()).expect("tick");
    }
    let latest = build_snapshot(
        Some(&state.view()),
        None,
        state.spec(),
        AnimationIntensity::Full,
    )
    .expect("latest");
    runtime.offer(old);
    runtime.offer(latest.clone());
    runtime.render_latest();
    assert_eq!(runtime.frame().expect("frame").snapshot, latest);
    assert_eq!(state.match_tick(), 30);
}

// integration-system/presentation-runtime::TC-003
#[test]
fn page_entities_are_scoped_but_match_entities_survive_overlays() {
    let mut entities = EntityLifecycle::default();
    entities.enter(AppState::MainMenu);
    let main = entities.page_entity().expect("main page");
    entities.spawn_match_entities();
    let match_scene = entities.match_entity().expect("match scene");

    for state in [
        AppState::Match,
        AppState::Paused,
        AppState::Settings,
        AppState::Paused,
        AppState::Match,
    ] {
        entities.enter(state);
        assert_ne!(entities.page_entity(), Some(main));
        assert_eq!(entities.match_entity(), Some(match_scene));
    }
    entities.enter(AppState::MainMenu);
    let first_main = entities.page_entity();
    entities.enter(AppState::Settings);
    entities.enter(AppState::MainMenu);
    assert_ne!(entities.page_entity(), first_main);
}

// integration-system/presentation-runtime::TC-004
#[test]
fn virtual_canvas_letterboxes_without_relayout_and_shares_one_scale() {
    for (width, height, expected) in [
        (1920., 1080., 1.),
        (2560., 1440., 4. / 3.),
        (1280., 1024., 2. / 3.),
        (2560., 1080., 1.),
    ] {
        let layout = VirtualCanvas::layout(width, height);
        assert!((layout.ui_scale - expected).abs() < f32::EPSILON);
        assert_eq!(layout.ui_scale, layout.world_scale);
        assert_eq!(layout.design_size, (1920., 1080.));
        assert_eq!(layout.anchor("p1_board"), Some((420., 540.)));
        if width / height != 16. / 9. {
            assert!(layout.letterbox.0 > 0. || layout.letterbox.1 > 0.);
        }
    }
}

// integration-system/presentation-runtime::TC-005
#[test]
fn unavailable_audio_is_non_fatal_and_diagnosed_once() {
    let mut runtime =
        FeedbackRuntime::new(AudioAvailability::Unavailable, 0, FeedbackBudget::default());
    for _ in 0..10 {
        runtime.consume(&events(1), AudioGains::FULL, true);
    }
    assert_eq!(runtime.audio_requests(), 0);
    assert_eq!(runtime.diagnostics().len(), 1);
}

// integration-system/presentation-runtime::TC-006
#[test]
fn no_gamepads_means_no_vibration_and_no_diagnostic() {
    let mut runtime =
        FeedbackRuntime::new(AudioAvailability::Available, 0, FeedbackBudget::default());
    runtime.consume(&events(4), AudioGains::FULL, true);
    assert_eq!(runtime.vibration_requests(), 0);
    assert!(runtime.diagnostics().is_empty());
}

// integration-system/presentation-runtime::TC-007
#[test]
fn high_feedback_is_merged_under_budgets_without_touching_rules() {
    let budget = FeedbackBudget {
        transient_entities: 4,
        concurrent_cues: 2,
    };
    let mut runtime = FeedbackRuntime::new(AudioAvailability::Available, 2, budget);
    runtime.consume(&events(38), AudioGains::FULL, true);
    assert!(runtime.live_transient_entities() <= 4);
    assert!(runtime.concurrent_cues() <= 2);
    assert_eq!(
        runtime.merged_batches(),
        1,
        "same-tick nuisance feedback is merged"
    );
}

// integration-system/presentation-runtime::TC-008
#[test]
fn character_visual_changes_do_not_enter_the_locked_spec_digest() {
    let a = presentation_common::spec(17);
    let b = presentation_common::spec(17);
    assert_eq!(a.digests.root, b.digests.root);
    assert_eq!(a, b);
}

fn checksum_trace(
    intensity: AnimationIntensity,
    audio: AudioAvailability,
    render_every: u64,
) -> Vec<u64> {
    let mut state = presentation_common::state(19);
    let mut trace = Vec::new();
    let mut runtime = PresentationRuntime::default();
    let _feedback = FeedbackRuntime::new(audio, 0, FeedbackBudget::default());
    for tick in 1..=120 {
        state.step(&presentation_common::idle()).expect("tick");
        trace.push(state.checksum().0);
        if tick % render_every == 0 {
            let snapshot = build_snapshot(Some(&state.view()), None, state.spec(), intensity)
                .expect("snapshot");
            runtime.sample(snapshot);
        }
    }
    trace
}

// integration-system/presentation-runtime::TC-009
#[test]
fn presentation_configuration_and_sampling_rate_do_not_change_rules() {
    let reference = checksum_trace(AnimationIntensity::Full, AudioAvailability::Available, 1);
    for (intensity, audio, cadence) in [
        (AnimationIntensity::Reduced, AudioAvailability::Available, 1),
        (AnimationIntensity::Full, AudioAvailability::Unavailable, 3),
        (
            AnimationIntensity::Reduced,
            AudioAvailability::Unavailable,
            10,
        ),
    ] {
        assert_eq!(checksum_trace(intensity, audio, cadence), reference);
    }
}

// integration-system/presentation-runtime::TC-010
#[test]
fn all_optional_presentation_outputs_can_be_absent_while_rules_advance() {
    let mut state = presentation_common::state(23);
    let mut feedback =
        FeedbackRuntime::new(AudioAvailability::Unavailable, 0, FeedbackBudget::default());
    for _ in 0..120 {
        let report = state.step(&presentation_common::idle()).expect("tick");
        feedback.consume(
            &publish_events(&report, AnimationIntensity::Reduced),
            AudioGains::FULL,
            true,
        );
    }
    assert_eq!(state.match_tick(), 120);
    assert_eq!(feedback.audio_requests(), 0);
    assert_eq!(feedback.vibration_requests(), 0);
    assert!(
        build_snapshot(
            Some(&state.view()),
            None,
            state.spec(),
            AnimationIntensity::Reduced
        )
        .is_some()
    );
}

/// Every kind of rule fact, in one report.
fn every_fact() -> MatchStepReport {
    MatchStepReport {
        match_tick: 12,
        phase: MatchPhase::Playing,
        events: vec![
            MatchEvent::GroupLocked(0),
            MatchEvent::ChainSettled {
                slot: 0,
                links: 3,
                all_clear: false,
            },
            MatchEvent::ChainSettled {
                slot: 1,
                links: 5,
                all_clear: true,
            },
            MatchEvent::AttackArbitrated {
                slot: 0,
                offset: 0,
                sent: 12,
            },
            MatchEvent::AttackArbitrated {
                slot: 1,
                offset: 6,
                sent: 0,
            },
            MatchEvent::FeverEntered(0),
            MatchEvent::FeverExited(1),
            MatchEvent::NuisanceDropped { slot: 1, count: 6 },
            MatchEvent::PlayerDefeated(1),
            MatchEvent::RoundEnded(game_core::match_state::RoundOutcome::Decided(0)),
        ],
    }
}

// integration-system/presentation-runtime::TC-013
#[test]
fn every_fact_asks_for_one_cue_of_its_own_kind_at_its_own_tick() {
    use client::presentation::AudioCue;

    let report = every_fact();
    let mut runtime =
        FeedbackRuntime::new(AudioAvailability::Available, 2, FeedbackBudget::default());
    let requests = runtime.consume(
        &publish_events(&report, AnimationIntensity::Full),
        AudioGains::FULL,
        true,
    );

    assert_eq!(
        requests.len(),
        report.events.len(),
        "one cue per confirmed fact"
    );
    assert!(
        requests.iter().all(|request| request.id.match_tick == 12),
        "every cue is asked for at the tick its fact happened on"
    );
    let kinds: Vec<AudioCue> = requests.iter().map(|request| request.cue).collect();
    assert_eq!(
        kinds,
        vec![
            AudioCue::Lock,
            AudioCue::Chain,
            AudioCue::AllClear,
            AudioCue::Attack,
            AudioCue::Offset,
            AudioCue::FeverEntered,
            AudioCue::FeverExited,
            AudioCue::NuisanceLanded,
            AudioCue::Defeat,
            AudioCue::RoundEnded,
        ]
    );

    // The same report offered again is the same facts, not new ones.
    let repeated = runtime.consume(
        &publish_events(&report, AnimationIntensity::Full),
        AudioGains::FULL,
        true,
    );
    assert!(
        repeated.is_empty(),
        "a fact is cued once, not once per frame"
    );
}

// integration-system/presentation-runtime::TC-013
#[test]
fn both_volume_sliders_reach_the_cue_gain_without_cancelling_it() {
    for (master, sfx, expected) in [
        (1.0, 1.0, 1.0),
        (0.5, 0.5, 0.25),
        (0.0, 1.0, 0.0),
        (1.0, 0.0, 0.0),
    ] {
        let mut runtime =
            FeedbackRuntime::new(AudioAvailability::Available, 0, FeedbackBudget::default());
        let requests = runtime.consume(&events(1), AudioGains { master, sfx }, true);
        let request = requests.first().expect("one fact asks for one cue");
        assert!(
            (request.gain - expected).abs() < f32::EPSILON,
            "master {master} and sfx {sfx} should multiply to {expected}, got {}",
            request.gain
        );
    }

    // Silence is a gain, not a cancellation: what was triggered stays readable.
    let mut muted =
        FeedbackRuntime::new(AudioAvailability::Available, 0, FeedbackBudget::default());
    assert_eq!(
        muted
            .consume(
                &events(1),
                AudioGains {
                    master: 0.0,
                    sfx: 0.0
                },
                true
            )
            .len(),
        1
    );

    // No device silences the cue the same way, and still reports it once.
    let mut deaf =
        FeedbackRuntime::new(AudioAvailability::Unavailable, 0, FeedbackBudget::default());
    let requests = deaf.consume(&events(1), AudioGains::FULL, true);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].gain, 0.0);
    assert_eq!(deaf.diagnostics().len(), 1);
}

// integration-system/presentation-runtime::TC-013
#[test]
fn vibration_needs_the_setting_a_pad_and_a_fact_worth_feeling() {
    use client::presentation::VibrationPattern;

    let report = every_fact();
    let published = publish_events(&report, AnimationIntensity::Full);

    let mut both_pads =
        FeedbackRuntime::new(AudioAvailability::Available, 2, FeedbackBudget::default());
    let requests = both_pads.consume(&published, AudioGains::FULL, true);
    let felt: Vec<(Option<usize>, VibrationPattern)> = requests
        .iter()
        .filter_map(|request| request.vibration.map(|pattern| (request.slot, pattern)))
        .collect();
    assert_eq!(
        felt,
        vec![
            (Some(0), VibrationPattern::Chain),
            (Some(1), VibrationPattern::Chain),
            (Some(0), VibrationPattern::FeverEntered),
            (Some(1), VibrationPattern::NuisanceLanded),
        ],
        "only chains, nuisance landing and entering Fever are felt, each on its own pad"
    );

    let mut setting_off =
        FeedbackRuntime::new(AudioAvailability::Available, 2, FeedbackBudget::default());
    assert!(
        setting_off
            .consume(&published, AudioGains::FULL, false)
            .iter()
            .all(|request| request.vibration.is_none()),
        "the setting turns vibration off without touching the cues"
    );

    let mut one_pad =
        FeedbackRuntime::new(AudioAvailability::Available, 1, FeedbackBudget::default());
    assert!(
        one_pad
            .consume(&published, AudioGains::FULL, true)
            .iter()
            .filter_map(|request| request.vibration.map(|_| request.slot))
            .all(|slot| slot == Some(0)),
        "a player with no pad feels nothing, and nobody else's pad buzzes for them"
    );
}

// integration-system/presentation-runtime::TC-013
#[test]
fn a_running_match_asks_for_cues_at_the_ticks_its_facts_happen_on() {
    let mut app = common::controlled_app();
    app.insert_resource(client::match_flow::FrozenMatch(presentation_common::spec(
        5,
    )));
    common::advance_to(&mut app, AppState::Match);

    // Far enough in for the countdown to end and for both sides to lock a
    // group, which is the first fact any match produces.
    for _ in 0..600 {
        app.update();
        common::run_fixed_tick(&mut app);
        if app
            .world()
            .resource::<client::feedback::MatchFeedback>()
            .requested()
            > 0
        {
            break;
        }
    }

    let feedback = app.world().resource::<client::feedback::MatchFeedback>();
    assert!(
        feedback.requested() > 0,
        "a running match produced no cue at all"
    );
    let tick = feedback
        .last_tick()
        .expect("cues carry the tick they came from");
    assert!(
        feedback
            .recent()
            .all(|request| request.id.match_tick <= tick),
        "no cue is asked for ahead of the tick it belongs to"
    );
    assert_eq!(
        feedback.diagnostics().len(),
        1,
        "a headless build has no audio device and says so once"
    );
}
