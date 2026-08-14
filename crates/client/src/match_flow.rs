//! Freezing a match specification and requesting the transition into `Match`.
//!
//! This is the consumer side of the rules data's blocking level: rule data is
//! the authority a match is played under, so a failed resolution means the
//! request is never made rather than that a match starts on substitute data.

use bevy::prelude::*;
use game_core::{
    MatchState,
    config::{CharacterId, RuleProfileId},
    match_spec::{LockedMatchSpec, MatchRequest},
};

use crate::app_state::CommittedTransition;
use crate::app_state::{AppState, AppTransitionCause, AppTransitionRequests, AppTransitionSet};
use crate::data::RulesData;
use crate::simulation::{FixedGameSet, RulesSimulation};

/// What the character-selection flow has confirmed.
#[derive(Debug, Clone, Resource)]
pub struct MatchSelection {
    /// Profile the match is played under.
    pub rule_profile_id: RuleProfileId,
    /// Seed for every named random stream.
    pub root_seed: u64,
    /// One character per participant slot.
    pub characters: [CharacterId; 2],
    /// Whether the selection has been confirmed by both sides.
    pub confirmed: bool,
}

/// The frozen specification the next match instance will be built from.
///
/// Match scoped: it is consumed by the `Match` entry that instantiates it and
/// released with the rest of the match, so a frozen spec never outlives the
/// match it was frozen for and never seeds a later one.
#[derive(Debug, Resource)]
pub struct FrozenMatch(pub LockedMatchSpec);

/// Source of root seeds for locally started matches.
///
/// Every seed is a pure function of the value the source was created with, so a
/// headless test can pin the whole sequence while production seeds it once.
#[derive(Debug, Default, Resource)]
pub struct MatchSeedSource(u64);

impl MatchSeedSource {
    /// Starts the sequence from a caller-chosen value.
    #[must_use]
    pub const fn new(state: u64) -> Self {
        Self(state)
    }

    /// Takes the next root seed, advancing the sequence.
    pub fn next_seed(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut seed = self.0;
        seed ^= seed >> 30;
        seed = seed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        seed ^= seed >> 27;
        seed = seed.wrapping_mul(0x94d0_49bb_1331_11eb);
        seed ^ (seed >> 31)
    }
}

/// The result page asking for another match under the same selection.
///
/// Inserted when 「再来一局」 is confirmed and consumed by [`request_rematch`],
/// which does the re-freeze the `RematchRequested` cause requires.
#[derive(Debug, Default, Resource)]
pub struct RematchIntent;

/// Why a confirmed selection did not start a match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchStartBlocked {
    /// The rules resolution has not settled yet.
    RulesPending,
    /// Rule data failed to load, so there is no authority to play under.
    RulesUnavailable(String),
    /// The selection could not be frozen.
    FreezeFailed(String),
}

/// Diagnostics for blocked match starts, so the reason stays observable.
#[derive(Debug, Default, Resource)]
pub struct MatchStartDiagnostics(pub Vec<MatchStartBlocked>);

/// Stable identity of the currently running rules instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct MatchInstanceId(pub u64);

#[derive(Debug, Default, Resource)]
struct NextMatchInstanceId(u64);

/// Placeholder for AI planning state whose lifetime is exactly one match instance.
#[derive(Debug, Default, Resource)]
pub struct AiPlanState;

/// Placeholder for resident board/HUD entities owned by a match instance.
#[derive(Debug, Default, Resource)]
pub struct MatchPresentationResources;

/// Why an already committed Match entry could not create a usable instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchLifecycleFailure {
    MissingFrozenMatch(AppTransitionCause),
    MissingResumedInstance,
}

#[derive(Debug, Default, Resource)]
pub struct MatchLifecycleDiagnostics(pub Vec<MatchLifecycleFailure>);

#[derive(Debug, Default, Resource)]
struct MatchCompletionReported(bool);

/// Freezes the selection and requests `CharacterSelect -> Match`.
///
/// Nothing is requested while the rules resolution is `Failed`: the match entry
/// is simply unreachable, and the rest of the client keeps working.
pub fn request_match_start(
    state: Res<State<AppState>>,
    selection: Option<Res<MatchSelection>>,
    rules: Option<Res<RulesData>>,
    frozen: Option<Res<FrozenMatch>>,
    mut requests: ResMut<AppTransitionRequests>,
    mut diagnostics: ResMut<MatchStartDiagnostics>,
    mut commands: Commands,
) {
    if *state.get() != AppState::CharacterSelect || frozen.is_some() {
        return;
    }
    let Some(selection) = selection.filter(|selection| selection.confirmed) else {
        return;
    };
    if let Some(spec) = freeze_selection(&selection, selection.root_seed, rules, &mut diagnostics) {
        commands.insert_resource(FrozenMatch(spec));
        requests.submit(AppState::Match, AppTransitionCause::CharacterConfirmed);
    }
}

/// Re-freezes the same selection under a new local seed and requests
/// `Result -> Match`.
///
/// `RematchRequested` means a new match rather than a replay of the old one, so
/// the freeze happens here, before the request is made, exactly as it does for
/// `CharacterConfirmed`. Without it the entry would fall back to whatever spec
/// was still around and silently replay the previous seed.
///
/// Reaching `Result` with a [`RematchIntent`] is a run condition rather than an
/// in-body check, so a caller registering this system has to state both.
pub fn request_rematch(
    selection: Option<Res<MatchSelection>>,
    rules: Option<Res<RulesData>>,
    mut seeds: ResMut<MatchSeedSource>,
    mut requests: ResMut<AppTransitionRequests>,
    mut diagnostics: ResMut<MatchStartDiagnostics>,
    mut commands: Commands,
) {
    let Some(selection) = selection else {
        return;
    };
    commands.remove_resource::<RematchIntent>();
    if let Some(spec) = freeze_selection(&selection, seeds.next_seed(), rules, &mut diagnostics) {
        commands.insert_resource(FrozenMatch(spec));
        requests.submit(AppState::Match, AppTransitionCause::RematchRequested);
    }
}

/// Freezes `selection` under `root_seed`, recording why if it cannot.
fn freeze_selection(
    selection: &MatchSelection,
    root_seed: u64,
    rules: Option<Res<RulesData>>,
    diagnostics: &mut MatchStartDiagnostics,
) -> Option<LockedMatchSpec> {
    let Some(rules) = rules else {
        diagnostics.0.push(MatchStartBlocked::RulesPending);
        return None;
    };
    let Some(library) = rules.rules() else {
        let reason = rules.error().map_or_else(
            || "rules unavailable".to_owned(),
            |error| format!("{error:?}"),
        );
        diagnostics
            .0
            .push(MatchStartBlocked::RulesUnavailable(reason));
        return None;
    };

    let request = MatchRequest {
        rule_profile_id: selection.rule_profile_id.clone(),
        root_seed,
        characters: selection.characters.clone(),
    };
    match LockedMatchSpec::freeze(request, library) {
        Ok(spec) => Some(spec),
        Err(error) => {
            diagnostics
                .0
                .push(MatchStartBlocked::FreezeFailed(error.to_string()));
            None
        }
    }
}

/// Registers the match-start request path.
#[derive(Debug, Default)]
pub struct MatchFlowPlugin;

impl Plugin for MatchFlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MatchStartDiagnostics>()
            .init_resource::<MatchLifecycleDiagnostics>()
            .init_resource::<NextMatchInstanceId>()
            .init_resource::<MatchCompletionReported>()
            .init_resource::<MatchSeedSource>()
            .add_systems(
                Update,
                (
                    request_match_start,
                    request_rematch.run_if(
                        in_state(AppState::Result).and_then(resource_exists::<RematchIntent>),
                    ),
                )
                    .in_set(AppTransitionSet::Request),
            )
            .add_systems(OnEnter(AppState::Match), enter_match)
            .add_systems(OnExit(AppState::Match), exit_match)
            .add_systems(OnExit(AppState::Paused), exit_paused)
            .add_systems(
                FixedUpdate,
                request_result_for_completed_match.after(FixedGameSet::Rules),
            );
    }
}

fn enter_match(
    transition: Option<Res<CommittedTransition>>,
    frozen: Option<Res<FrozenMatch>>,
    current: Option<Res<RulesSimulation>>,
    mut next_id: ResMut<NextMatchInstanceId>,
    mut completion: ResMut<MatchCompletionReported>,
    mut diagnostics: ResMut<MatchLifecycleDiagnostics>,
    mut commands: Commands,
) {
    let Some(transition) = transition else {
        diagnostics
            .0
            .push(MatchLifecycleFailure::MissingFrozenMatch(
                AppTransitionCause::CharacterConfirmed,
            ));
        return;
    };

    if transition.cause == AppTransitionCause::ResumeRequested {
        if current.is_none() {
            diagnostics
                .0
                .push(MatchLifecycleFailure::MissingResumedInstance);
        }
        return;
    }

    let spec = if transition.cause == AppTransitionCause::RestartRequested {
        current
            .as_ref()
            .map(|simulation| simulation.0.spec().clone())
    } else {
        frozen.as_ref().map(|frozen| frozen.0.clone())
    };
    let Some(spec) = spec else {
        release_match_scoped(&mut commands);
        diagnostics
            .0
            .push(MatchLifecycleFailure::MissingFrozenMatch(transition.cause));
        return;
    };

    next_id.0 += 1;
    completion.0 = false;
    // The instance owns its spec from here on, and `RestartRequested` reads it
    // back from there; keeping the frozen copy would only let it seed a match
    // it was never frozen for.
    commands.remove_resource::<FrozenMatch>();
    commands.insert_resource(RulesSimulation(MatchState::new(spec)));
    commands.insert_resource(MatchInstanceId(next_id.0));
    commands.insert_resource(AiPlanState);
    commands.insert_resource(MatchPresentationResources);
}

fn release_match_scoped(commands: &mut Commands) {
    commands.remove_resource::<FrozenMatch>();
    commands.remove_resource::<RulesSimulation>();
    commands.remove_resource::<MatchInstanceId>();
    commands.remove_resource::<AiPlanState>();
    commands.remove_resource::<MatchPresentationResources>();
}

/// What the result page shows about the match that just ended.
///
/// Captured before the instance is released, because by design the rules
/// instance does not survive leaving `Match` -- and the result page is exactly
/// the screen that has to outlive it.
#[derive(Debug, Clone, Copy, Resource)]
pub struct MatchResultSummary {
    /// Rounds won per participant slot.
    pub wins: [u8; 2],
    /// Participant that reached two wins, when the match ran to completion.
    pub winner: Option<usize>,
}

fn exit_match(
    transition: Option<Res<CommittedTransition>>,
    simulation: Option<Res<RulesSimulation>>,
    mut commands: Commands,
) {
    if transition
        .as_ref()
        .is_some_and(|transition| transition.to == AppState::Paused)
    {
        return;
    }
    if let Some(simulation) = simulation {
        let view = simulation.0.view();
        commands.insert_resource(MatchResultSummary {
            wins: view.wins,
            winner: match view.phase {
                game_core::match_state::MatchPhase::Completed(outcome) => Some(outcome.winner),
                _ => None,
            },
        });
    }
    release_match_scoped(&mut commands);
}

fn exit_paused(transition: Option<Res<CommittedTransition>>, mut commands: Commands) {
    if transition
        .as_ref()
        .is_none_or(|transition| transition.to != AppState::MainMenu)
    {
        return;
    }
    release_match_scoped(&mut commands);
}

fn request_result_for_completed_match(
    simulation: Option<Res<RulesSimulation>>,
    mut completion: ResMut<MatchCompletionReported>,
    mut requests: ResMut<AppTransitionRequests>,
) {
    if completion.0 {
        return;
    }
    if simulation.is_some_and(|simulation| simulation.0.outcome().is_some()) {
        completion.0 = true;
        requests.submit(AppState::Result, AppTransitionCause::MatchCompleted);
    }
}
