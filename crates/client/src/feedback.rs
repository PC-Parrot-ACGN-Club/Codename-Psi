//! The disposable half of the presentation layer: audio cues and vibration.
//!
//! R1 ships no samples, so nothing here makes a sound. What it does is turn
//! each confirmed rule fact into exactly one cue request at exactly one tick,
//! which is the part later audio work plugs into unchanged.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::app_state::AppState;
use crate::presentation::{
    AudioAvailability, AudioGains, CueRequest, FeedbackBudget, FeedbackRuntime, publish_events,
};
use crate::settings::UserSettings;
use crate::simulation::LatestStepReport;

#[derive(Debug, Default)]
pub struct FeedbackPlugin;

impl Plugin for FeedbackPlugin {
    fn build(&self, app: &mut App) {
        // A build without an audio device is the normal headless case, not a
        // failure: the cue path still runs, at zero gain.
        let audio = if app.is_plugin_added::<bevy::audio::AudioPlugin>() {
            AudioAvailability::Available
        } else {
            AudioAvailability::Unavailable
        };
        app.insert_resource(MatchFeedback::new(audio))
            .add_systems(
                Update,
                request_cues.run_if(in_state(AppState::Match).or_else(in_state(AppState::Result))),
            )
            .add_systems(OnExit(AppState::Result), reset_feedback);
    }
}

/// How many recent requests stay readable.
///
/// A match produces cues for as long as it runs, so the log is a window rather
/// than a history: it exists to make the current tick's cues observable, and a
/// full history would grow without bound at 60 Hz.
pub const CUE_LOG_LEN: usize = 64;

/// The cue runtime and the requests it has most recently made.
#[derive(Resource)]
pub struct MatchFeedback {
    runtime: FeedbackRuntime,
    recent: VecDeque<CueRequest>,
    requested: usize,
    /// Rule tick the last batch of requests came from.
    last_tick: Option<u64>,
}

impl MatchFeedback {
    fn new(audio: AudioAvailability) -> Self {
        Self {
            runtime: FeedbackRuntime::new(audio, 0, FeedbackBudget::default()),
            recent: VecDeque::new(),
            requested: 0,
            last_tick: None,
        }
    }

    /// The most recent cue requests, oldest first.
    pub fn recent(&self) -> impl Iterator<Item = &CueRequest> {
        self.recent.iter()
    }

    /// Every cue requested since the match began, including those aged out.
    #[must_use]
    pub const fn requested(&self) -> usize {
        self.requested
    }

    /// The rule tick the last requests were made for.
    #[must_use]
    pub const fn last_tick(&self) -> Option<u64> {
        self.last_tick
    }

    /// Diagnostics the runtime has recorded, such as a missing audio device.
    #[must_use]
    pub fn diagnostics(&self) -> &[String] {
        self.runtime.diagnostics()
    }

    fn record(&mut self, tick: u64, requests: Vec<CueRequest>) {
        if requests.is_empty() {
            return;
        }
        self.last_tick = Some(tick);
        self.requested += requests.len();
        for request in requests {
            if self.recent.len() == CUE_LOG_LEN {
                self.recent.pop_front();
            }
            self.recent.push_back(request);
        }
    }
}

/// Ask for this tick's cues, once per confirmed fact.
///
/// The report is read rather than the snapshot: a cue belongs to the fact that
/// produced it, and the runtime's own de-duplication is what keeps a frame that
/// sees the same report twice from playing it twice.
fn request_cues(
    report: Res<LatestStepReport>,
    settings: Res<UserSettings>,
    pads: Query<(), With<Gamepad>>,
    mut feedback: ResMut<MatchFeedback>,
) {
    let Some(report) = report.0.as_ref() else {
        return;
    };
    let events = publish_events(report, settings.animation_intensity);
    let gains = AudioGains {
        master: settings.master_volume,
        sfx: settings.sfx_volume,
    };
    feedback.runtime.set_gamepads(pads.iter().count());
    let requests = feedback.runtime.consume(&events, gains, settings.vibration);
    feedback.record(report.match_tick, requests);
}

/// Start the next match with no cue history and no consumed identifiers.
fn reset_feedback(mut commands: Commands, feedback: Res<MatchFeedback>) {
    let audio = feedback.runtime.audio();
    commands.insert_resource(MatchFeedback::new(audio));
}
