//! Immutable rule projection selected before a BO3 begins.

use crate::{
    board::BoardGeometry,
    config::{CharacterId, ConfigError, DropSet, RuleProfile, RuleProfileId, ValidatedRuleLibrary},
    resolution::ResolutionRules,
    rules::ChainPowerProfile,
};

/// The two participant slots supported by one R1 match.
pub const PARTICIPANT_SLOTS: usize = 2;

/// Request made by the client after both character selections are confirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchRequest {
    pub rule_profile_id: RuleProfileId,
    pub root_seed: u64,
    pub characters: [CharacterId; PARTICIPANT_SLOTS],
}

/// Immutable data needed by a running match; it never observes asset reloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedMatchSpec {
    pub profile_id: RuleProfileId,
    pub rule_version: String,
    pub root_seed: u64,
    pub board_geometry: BoardGeometry,
    pub color_count: u8,
    pub resolution: ResolutionRules,
    pub nuisance_queue_limit: u32,
    pub nuisance_drop_limit: u32,
    pub fever_capacity: u8,
    pub fever_initial_time_ticks: u32,
    pub fever_min_time_ticks: u32,
    pub fever_max_time_ticks: u32,
    pub characters: [CharacterId; PARTICIPANT_SLOTS],
    pub chain_power: [ChainPowerProfile; PARTICIPANT_SLOTS],
    pub drop_sets: [DropSet; PARTICIPANT_SLOTS],
}

impl LockedMatchSpec {
    /// Freezes exactly the data selected by a request.
    pub fn freeze(
        request: MatchRequest,
        library: &ValidatedRuleLibrary,
    ) -> Result<Self, ConfigError> {
        let profile = library.profile(&request.rule_profile_id).ok_or_else(|| {
            ConfigError::InvalidData(format!(
                "unknown rule profile {}",
                request.rule_profile_id.0
            ))
        })?;
        let power_for = |character: &CharacterId| {
            library
                .character_play(&request.rule_profile_id, character)
                .ok_or_else(|| {
                    ConfigError::InvalidData(format!(
                        "missing gameplay data for character {}",
                        character.0
                    ))
                })?
                .chain_power_for_match()
        };
        let chain_power = [
            power_for(&request.characters[0])?,
            power_for(&request.characters[1])?,
        ];
        let drop_for = |character: &CharacterId| {
            library
                .character_play(&request.rule_profile_id, character)
                .ok_or_else(|| {
                    ConfigError::InvalidData(format!(
                        "missing gameplay data for character {}",
                        character.0
                    ))
                })
                .map(|play| play.drop_set.clone())
        };
        let drop_sets = [
            drop_for(&request.characters[0])?,
            drop_for(&request.characters[1])?,
        ];
        let board_geometry = profile.field.geometry().ok_or_else(|| {
            ConfigError::InvalidData("validated profile has invalid board geometry".into())
        })?;
        Ok(Self {
            profile_id: request.rule_profile_id,
            rule_version: profile.rule_version.clone(),
            root_seed: request.root_seed,
            board_geometry,
            color_count: profile.field.color_count,
            resolution: resolution_rules(profile),
            nuisance_queue_limit: profile.nuisance.queue_limit,
            nuisance_drop_limit: profile.nuisance.drop_limit,
            fever_capacity: profile.fever.gauge_capacity,
            fever_initial_time_ticks: profile.fever.initial_time_ticks,
            fever_min_time_ticks: profile.fever.min_time_ticks,
            fever_max_time_ticks: profile.fever.max_time_ticks,
            characters: request.characters,
            chain_power,
            drop_sets,
        })
    }
}

fn resolution_rules(profile: &RuleProfile) -> ResolutionRules {
    ResolutionRules {
        clear_preview_ticks: profile.resolve.clear_preview_ticks,
        gravity_ticks_by_distance: profile.resolve.gravity_ticks_by_distance.clone(),
        clear_threshold: profile.resolve.clear_threshold,
    }
}
