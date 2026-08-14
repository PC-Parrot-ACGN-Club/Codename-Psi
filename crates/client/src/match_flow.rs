//! Freezing a match specification and requesting the transition into `Match`.
//!
//! This is the consumer side of the rules data's blocking level: rule data is
//! the authority a match is played under, so a failed resolution means the
//! request is never made rather than that a match starts on substitute data.

use bevy::prelude::*;
use game_core::{
    config::{CharacterId, RuleProfileId},
    match_spec::{LockedMatchSpec, MatchRequest},
};

use crate::app_state::{AppState, AppTransitionCause, AppTransitionRequests, AppTransitionSet};
use crate::data::RulesData;

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

/// The frozen specification the match will run under.
#[derive(Debug, Resource)]
pub struct FrozenMatch(pub LockedMatchSpec);

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
    let Some(rules) = rules else {
        diagnostics.0.push(MatchStartBlocked::RulesPending);
        return;
    };
    let Some(library) = rules.rules() else {
        let reason = rules.error().map_or_else(
            || "rules unavailable".to_owned(),
            |error| format!("{error:?}"),
        );
        diagnostics
            .0
            .push(MatchStartBlocked::RulesUnavailable(reason));
        return;
    };

    let request = MatchRequest {
        rule_profile_id: selection.rule_profile_id.clone(),
        root_seed: selection.root_seed,
        characters: selection.characters.clone(),
    };
    match LockedMatchSpec::freeze(request, library) {
        Ok(spec) => {
            commands.insert_resource(FrozenMatch(spec));
            requests.submit(AppState::Match, AppTransitionCause::MatchStartRequested);
        }
        Err(error) => {
            diagnostics
                .0
                .push(MatchStartBlocked::FreezeFailed(error.to_string()));
        }
    }
}

/// Registers the match-start request path.
#[derive(Debug, Default)]
pub struct MatchFlowPlugin;

impl Plugin for MatchFlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MatchStartDiagnostics>().add_systems(
            Update,
            request_match_start.in_set(AppTransitionSet::Request),
        );
    }
}
