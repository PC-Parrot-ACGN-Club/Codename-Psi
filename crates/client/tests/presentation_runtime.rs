//! In-memory presentation runtime coverage from `integration-system/presentation-runtime.md`.

mod presentation_common;

use client::app_state::AppState;
use client::presentation::{
    AudioAvailability, EntityLifecycle, FeedbackBudget, FeedbackRuntime, PresentationRuntime,
    VirtualCanvas, build_snapshot, publish_events,
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
        runtime.consume(&events(1));
    }
    assert_eq!(runtime.audio_requests(), 0);
    assert_eq!(runtime.diagnostics().len(), 1);
}

// integration-system/presentation-runtime::TC-006
#[test]
fn no_gamepads_means_no_vibration_and_no_diagnostic() {
    let mut runtime =
        FeedbackRuntime::new(AudioAvailability::Available, 0, FeedbackBudget::default());
    runtime.consume(&events(4));
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
    runtime.consume(&events(38));
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
        feedback.consume(&publish_events(&report, AnimationIntensity::Reduced));
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
