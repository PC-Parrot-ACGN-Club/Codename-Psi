//! Versioned config stubs parsed from in-memory RON / JSON.
//!
//! Full rule profiles replace these stubs when the deterministic kernel lands.

use std::collections::BTreeMap;

use serde::Deserialize;
use thiserror::Error;

use crate::{
    board::BoardGeometry,
    rules::{CHAIN_POWER_TABLE_LEN, ChainPowerProfile},
};

/// Schema supported by the versioned Fever rule documents.
pub const RULE_PROFILE_SCHEMA_VERSION: u32 = 1;

/// A stable profile identity kept separate from the version and digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct RuleProfileId(pub String);

/// A stable character identity used in match requests.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct CharacterId(pub String);

/// The versioned rule portion of a match's content library.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RuleProfile {
    pub schema_version: u32,
    pub id: RuleProfileId,
    pub rule_version: String,
    pub reference_profile: String,
    pub field: FieldConfig,
    pub resolve: ResolveConfig,
    pub nuisance: NuisanceConfig,
    pub fever: FeverConfig,
}

/// Geometry and color count used by all fields in this profile.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FieldConfig {
    pub width: u8,
    pub height: u8,
    pub hidden_rows: u8,
    pub spawn_column: u8,
    pub color_count: u8,
}

impl FieldConfig {
    /// Converts validated config coordinates to rules-board geometry.
    #[must_use]
    pub fn geometry(&self) -> Option<BoardGeometry> {
        BoardGeometry::new(self.width, self.height, self.hidden_rows, self.spawn_column)
    }
}

/// Tick values used by the resolution component.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ResolveConfig {
    pub clear_threshold: u8,
    pub clear_preview_ticks: u16,
    pub gravity_ticks_by_distance: Vec<u16>,
}

/// Exact nuisance queue limits frozen into a profile.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NuisanceConfig {
    pub queue_limit: u32,
    pub drop_limit: u32,
}

/// The Fever values needed before the game-mode state is constructed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FeverConfig {
    pub gauge_capacity: u8,
    pub initial_time_ticks: u32,
    pub min_time_ticks: u32,
    pub max_time_ticks: u32,
    pub min_level: u8,
    pub max_level: u8,
}

/// Per-character gameplay data in one profile partition.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CharacterPlay {
    pub schema_version: u32,
    pub profile_id: RuleProfileId,
    pub character_id: CharacterId,
    #[serde(default)]
    pub drop_set: DropSet,
    pub normal_chain_power: Vec<u16>,
    pub fever_chain_power: Vec<u16>,
}

/// Sixteen configured group templates for one character.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DropSet(pub Vec<DropTemplate>);

impl Default for DropSet {
    fn default() -> Self {
        Self(vec![DropTemplate::pair(); 16])
    }
}

/// One group expressed around its spawn pivot.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DropTemplate {
    pub balls: Vec<DropBallTemplate>,
}

impl DropTemplate {
    fn pair() -> Self {
        Self {
            balls: vec![
                DropBallTemplate { dx: 0, dy: 0 },
                DropBallTemplate { dx: 0, dy: -1 },
            ],
        }
    }
}

/// A ball position whose color comes from the participant's color stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct DropBallTemplate {
    pub dx: i8,
    pub dy: i8,
}

impl CharacterPlay {
    fn chain_power(&self) -> Result<ChainPowerProfile, ConfigError> {
        let normal: [u16; CHAIN_POWER_TABLE_LEN] = self
            .normal_chain_power
            .clone()
            .try_into()
            .map_err(|values: Vec<u16>| {
                ConfigError::InvalidData(format!(
                    "normal_chain_power must contain {CHAIN_POWER_TABLE_LEN} values, got {}",
                    values.len()
                ))
            })?;
        let fever: [u16; CHAIN_POWER_TABLE_LEN] = self
            .fever_chain_power
            .clone()
            .try_into()
            .map_err(|values: Vec<u16>| {
                ConfigError::InvalidData(format!(
                    "fever_chain_power must contain {CHAIN_POWER_TABLE_LEN} values, got {}",
                    values.len()
                ))
            })?;
        ChainPowerProfile::new(normal, fever)
            .map_err(|error| ConfigError::InvalidData(error.to_string()))
    }

    /// Converts the validated stored samples to runtime-authoritative curves.
    pub(crate) fn chain_power_for_match(&self) -> Result<ChainPowerProfile, ConfigError> {
        self.chain_power()
    }
}

/// Validated in-memory content; callers cannot freeze a match from raw files.
#[derive(Debug, Clone)]
pub struct ValidatedRuleLibrary {
    profiles: BTreeMap<RuleProfileId, RuleProfile>,
    plays: BTreeMap<(RuleProfileId, CharacterId), CharacterPlay>,
}

impl ValidatedRuleLibrary {
    /// Validates independently parsed profile and character-play documents.
    pub fn new(profiles: Vec<RuleProfile>, plays: Vec<CharacterPlay>) -> Result<Self, ConfigError> {
        let mut profile_map = BTreeMap::new();
        for profile in profiles {
            validate_profile(&profile)?;
            if profile_map.insert(profile.id.clone(), profile).is_some() {
                return Err(ConfigError::InvalidData("duplicate rule profile id".into()));
            }
        }
        let mut play_map = BTreeMap::new();
        for play in plays {
            if !profile_map.contains_key(&play.profile_id) {
                return Err(ConfigError::InvalidData(format!(
                    "play references unknown profile {}",
                    play.profile_id.0
                )));
            }
            play.chain_power()?;
            validate_drop_set(&play)?;
            let key = (play.profile_id.clone(), play.character_id.clone());
            if play_map.insert(key, play).is_some() {
                return Err(ConfigError::InvalidData(
                    "duplicate character play definition".into(),
                ));
            }
        }
        Ok(Self {
            profiles: profile_map,
            plays: play_map,
        })
    }

    /// Returns a profile selected by id.
    #[must_use]
    pub fn profile(&self, id: &RuleProfileId) -> Option<&RuleProfile> {
        self.profiles.get(id)
    }

    /// Returns validated gameplay data for a character under a profile.
    #[must_use]
    pub fn character_play(
        &self,
        profile: &RuleProfileId,
        character: &CharacterId,
    ) -> Option<&CharacterPlay> {
        self.plays.get(&(profile.clone(), character.clone()))
    }
}

fn validate_drop_set(play: &CharacterPlay) -> Result<(), ConfigError> {
    if play.drop_set.0.len() != 16 {
        return Err(ConfigError::InvalidData(
            "drop_set must contain 16 turns".into(),
        ));
    }
    for group in &play.drop_set.0 {
        if !(2..=4).contains(&group.balls.len()) {
            return Err(ConfigError::InvalidData(
                "drop group must contain 2 to 4 balls".into(),
            ));
        }
    }
    Ok(())
}

/// Parses one versioned rule profile from in-memory RON.
pub fn parse_rule_profile(source: &str) -> Result<RuleProfile, ConfigError> {
    let profile: RuleProfile =
        ron::from_str(source).map_err(|error| ConfigError::Ron(error.to_string()))?;
    if profile.schema_version != RULE_PROFILE_SCHEMA_VERSION {
        return Err(ConfigError::UnsupportedSchema {
            found: profile.schema_version,
            supported: RULE_PROFILE_SCHEMA_VERSION,
        });
    }
    validate_profile(&profile)?;
    Ok(profile)
}

/// Parses one versioned character gameplay document from in-memory RON.
pub fn parse_character_play(source: &str) -> Result<CharacterPlay, ConfigError> {
    let play: CharacterPlay =
        ron::from_str(source).map_err(|error| ConfigError::Ron(error.to_string()))?;
    if play.schema_version != RULE_PROFILE_SCHEMA_VERSION {
        return Err(ConfigError::UnsupportedSchema {
            found: play.schema_version,
            supported: RULE_PROFILE_SCHEMA_VERSION,
        });
    }
    play.chain_power()?;
    Ok(play)
}

fn validate_profile(profile: &RuleProfile) -> Result<(), ConfigError> {
    if profile.id.0.is_empty()
        || profile.rule_version.is_empty()
        || profile.reference_profile.is_empty()
    {
        return Err(ConfigError::InvalidData(
            "profile identifiers must not be empty".into(),
        ));
    }
    if profile.field.color_count < 2 || profile.field.geometry().is_none() {
        return Err(ConfigError::InvalidData(
            "invalid field geometry or color count".into(),
        ));
    }
    if profile.resolve.clear_threshold < 2
        || profile.resolve.clear_preview_ticks == 0
        || profile.resolve.gravity_ticks_by_distance.len() < 2
        || profile.resolve.gravity_ticks_by_distance[0] != 0
        || profile.resolve.gravity_ticks_by_distance[1..]
            .iter()
            .any(|ticks| *ticks == 0)
    {
        return Err(ConfigError::InvalidData(
            "invalid resolution timing values".into(),
        ));
    }
    if profile.nuisance.queue_limit == 0
        || profile.nuisance.drop_limit == 0
        || profile.nuisance.drop_limit > profile.nuisance.queue_limit
    {
        return Err(ConfigError::InvalidData("invalid nuisance limits".into()));
    }
    if profile.fever.gauge_capacity == 0
        || profile.fever.min_time_ticks > profile.fever.initial_time_ticks
        || profile.fever.initial_time_ticks > profile.fever.max_time_ticks
        || profile.fever.min_level > profile.fever.max_level
    {
        return Err(ConfigError::InvalidData("invalid Fever range".into()));
    }
    Ok(())
}

/// Schema version supported by the current stub loaders.
pub const STUB_SCHEMA_VERSION: u32 = 1;

/// Minimal rules document used to prove the data loading path.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RulesStub {
    pub schema_version: u32,
    pub id: String,
}

/// Minimal localization table used to prove the i18n loading path.
///
/// Locale-set semantics belong to `client::i18n`; this stub only proves that a
/// versioned JSON document parses into typed data from an in-memory source.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct I18nStub {
    pub schema_version: u32,
    pub messages: BTreeMap<String, String>,
}

/// Typed failures for stub config parsing (development diagnostics).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("failed to parse RON rules stub: {0}")]
    Ron(String),
    #[error("failed to parse JSON i18n stub: {0}")]
    Json(String),
    #[error("unsupported schema_version {found} (supported: {supported})")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("invalid data: {0}")]
    InvalidData(String),
}

/// Parse a rules stub from a RON document string.
pub fn parse_rules_stub(ron_source: &str) -> Result<RulesStub, ConfigError> {
    let stub: RulesStub =
        ron::from_str(ron_source).map_err(|err| ConfigError::Ron(err.to_string()))?;
    ensure_schema(stub.schema_version)?;
    Ok(stub)
}

/// Parse an i18n stub from a JSON document string.
pub fn parse_i18n_stub(json_source: &str) -> Result<I18nStub, ConfigError> {
    let stub: I18nStub =
        serde_json::from_str(json_source).map_err(|err| ConfigError::Json(err.to_string()))?;
    ensure_schema(stub.schema_version)?;
    Ok(stub)
}

fn ensure_schema(found: u32) -> Result<(), ConfigError> {
    if found == STUB_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ConfigError::UnsupportedSchema {
            found,
            supported: STUB_SCHEMA_VERSION,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES_STUB: &str = include_str!("../../../assets/data/rules.stub.ron");
    const I18N_EN: &str = include_str!("../../../assets/i18n/en.json");
    const I18N_ZH: &str = include_str!("../../../assets/i18n/zh-CN.json");

    #[test]
    fn parses_rules_stub_from_assets() {
        let stub = parse_rules_stub(RULES_STUB).expect("rules stub should parse");
        assert_eq!(stub.schema_version, STUB_SCHEMA_VERSION);
        assert_eq!(stub.id, "stub");
    }

    #[test]
    fn parses_i18n_stubs_from_assets() {
        let en = parse_i18n_stub(I18N_EN).expect("en i18n stub should parse");
        let zh = parse_i18n_stub(I18N_ZH).expect("zh-CN i18n stub should parse");
        assert_eq!(en.schema_version, STUB_SCHEMA_VERSION);
        assert_eq!(zh.schema_version, STUB_SCHEMA_VERSION);
        assert_eq!(
            en.messages.get("app.title").map(String::as_str),
            Some("Codename Psi")
        );
        assert_eq!(
            zh.messages.get("app.title").map(String::as_str),
            Some("Codename Psi")
        );
    }

    #[test]
    fn rejects_unsupported_rules_schema() {
        let err = parse_rules_stub("(schema_version: 99, id: \"x\")\n")
            .expect_err("schema 99 should be rejected");
        assert_eq!(
            err,
            ConfigError::UnsupportedSchema {
                found: 99,
                supported: STUB_SCHEMA_VERSION,
            }
        );
    }

    #[test]
    fn rejects_invalid_ron() {
        let err = parse_rules_stub("not ron").expect_err("invalid ron should fail");
        assert!(matches!(err, ConfigError::Ron(_)));
    }

    #[test]
    fn rejects_invalid_json() {
        let err = parse_i18n_stub("{").expect_err("invalid json should fail");
        assert!(matches!(err, ConfigError::Json(_)));
    }
}
