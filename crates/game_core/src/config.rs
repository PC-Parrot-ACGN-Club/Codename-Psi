//! Versioned rule configuration parsed from in-memory RON / JSON.
//!
//! Configuration is split into two independently versioned parts. A **rule
//! profile** says how one set of competitive rules computes; a **content
//! library** (roster, per-character gameplay data, Fever puzzle books) says
//! what is selectable under that profile. Nothing here touches the filesystem:
//! callers pass already-read text in.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use thiserror::Error;

use crate::{
    board::BoardGeometry,
    digest::{ContentDigest, DigestWriter, Digestible, root_digest},
    rules::{CHAIN_POWER_TABLE_LEN, ChainPowerParameters, ChainPowerProfile},
};

/// Schema supported by the versioned rule documents.
pub const RULE_PROFILE_SCHEMA_VERSION: u32 = 1;

/// Number of hands in one drop-set cycle.
pub const DROP_SET_LEN: usize = 16;

/// A stable profile identity kept separate from the version and digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct RuleProfileId(pub String);

/// A stable character identity used in match requests.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct CharacterId(pub String);

impl Digestible for RuleProfileId {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.str(&self.0);
    }
}

impl Digestible for CharacterId {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.str(&self.0);
    }
}

/// Which layer of semantic validation rejected a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationLayer {
    /// Ids, references, ranges, table lengths and structural completeness.
    Integrity,
    /// Whether one profile's own values combine coherently.
    ProfileConsistency,
    /// Whether the content library covers what the profile declares.
    ContentCoverage,
}

impl std::fmt::Display for ValidationLayer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Integrity => "integrity",
            Self::ProfileConsistency => "profile-consistency",
            Self::ContentCoverage => "content-coverage",
        };
        formatter.write_str(name)
    }
}

/// Typed failures for config parsing and validation.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ConfigError {
    /// A RON document could not be deserialized.
    #[error("failed to parse RON document: {0}")]
    Ron(String),
    /// A JSON document could not be deserialized.
    #[error("failed to parse JSON document: {0}")]
    Json(String),
    /// The document declares a schema this build does not support.
    #[error("unsupported schema_version {found} (supported: {supported})")]
    UnsupportedSchema {
        /// Version found in the document.
        found: u32,
        /// Version this build supports.
        supported: u32,
    },
    /// A structurally valid document carries unusable data.
    #[error("invalid data: {0}")]
    InvalidData(String),
    /// A semantic constraint was violated, located to a field path.
    #[error("{layer} validation failed at {path}: {constraint}")]
    Validation {
        /// Which validation layer rejected the data.
        layer: ValidationLayer,
        /// Dotted path of the offending field.
        path: String,
        /// The constraint that was violated.
        constraint: String,
    },
}

fn integrity(path: impl Into<String>, constraint: impl Into<String>) -> ConfigError {
    ConfigError::Validation {
        layer: ValidationLayer::Integrity,
        path: path.into(),
        constraint: constraint.into(),
    }
}

fn consistency(path: impl Into<String>, constraint: impl Into<String>) -> ConfigError {
    ConfigError::Validation {
        layer: ValidationLayer::ProfileConsistency,
        path: path.into(),
        constraint: constraint.into(),
    }
}

fn coverage(path: impl Into<String>, constraint: impl Into<String>) -> ConfigError {
    ConfigError::Validation {
        layer: ValidationLayer::ContentCoverage,
        path: path.into(),
        constraint: constraint.into(),
    }
}

// ---------------------------------------------------------------------------
// Rule profile
// ---------------------------------------------------------------------------

/// One complete set of competitive rule values.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RuleProfile {
    /// Document schema version.
    pub schema_version: u32,
    /// Stable profile identity.
    pub id: RuleProfileId,
    /// Rule revision, versioned separately from the schema.
    pub rule_version: String,
    /// Which original profile the values were transcribed from.
    pub reference_profile: String,
    /// Board geometry and color count.
    pub field: FieldConfig,
    /// Supply and falling-group timing.
    pub drop: DropConfig,
    /// Rotation timing and push-back parameters.
    pub rotation: RotationConfig,
    /// Chain resolution phase timing.
    pub resolve: ResolveConfig,
    /// Score tables, target score and margin decay.
    pub scoring: ScoringConfig,
    /// Score-to-attack conversion rules.
    pub offense: OffenseConfig,
    /// Nuisance queue limits and drop geometry.
    pub nuisance: NuisanceConfig,
    /// Fever gauge, time and puzzle-level values.
    pub fever: FeverConfig,
}

/// Geometry and color count used by all fields in this profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct FieldConfig {
    /// Column count.
    pub width: u8,
    /// Row count including hidden rows.
    pub height: u8,
    /// Hidden rows at the top, excluded from resolution.
    pub hidden_rows: u8,
    /// Column a falling group spawns in.
    pub spawn_column: u8,
    /// Number of distinct ball colors.
    pub color_count: u8,
}

impl FieldConfig {
    /// Converts validated config coordinates to rules-board geometry.
    #[must_use]
    pub fn geometry(&self) -> Option<BoardGeometry> {
        BoardGeometry::new(self.width, self.height, self.hidden_rows, self.spawn_column)
    }

    /// Number of visible cells, i.e. the resolution region.
    #[must_use]
    pub const fn visible_cells(&self) -> u32 {
        (self.width as u32) * ((self.height - self.hidden_rows) as u32)
    }
}

/// Supply and falling-group timing, all in ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct DropConfig {
    /// How many upcoming groups the player can see.
    pub next_queue_len: u8,
    /// Ticks per cell of unassisted falling.
    pub natural_fall_ticks: u16,
    /// Ticks per cell while soft dropping.
    pub soft_drop_ticks: u16,
    /// Delay before a held horizontal direction repeats.
    pub horizontal_repeat_delay_ticks: u16,
    /// Interval between repeats of a held horizontal direction.
    pub horizontal_repeat_interval_ticks: u16,
    /// Minimum ticks between two horizontal moves.
    pub horizontal_cooldown_ticks: u16,
    /// Grounded ticks accumulated before a group locks.
    pub lock_delay_ticks: u16,
    /// How many rotation push-ups a group may take before locking.
    pub lift_limit: u8,
    /// Delay before the pivot ball begins its post-lock free fall.
    pub split_delay_pivot_ticks: u16,
    /// Delay before a follower ball begins its post-lock free fall.
    pub split_delay_follower_ticks: u16,
}

/// Rotation timing. The push-back judgement order itself is rule logic, not data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct RotationConfig {
    /// Minimum ticks between two rotations in the same direction.
    pub cooldown_ticks: u16,
    /// How many blocked attempts release a 180 degree flip.
    pub double_rotation_period: u8,
}

/// Tick values used by the resolution component.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ResolveConfig {
    /// Minimum connected-group size that clears.
    pub clear_threshold: u8,
    /// Preview duration before a clear commits.
    pub clear_preview_ticks: u16,
    /// Fall duration indexed by distance in cells; index zero must be zero.
    ///
    /// Post-lock split free fall shares this table, as the two use the same
    /// parameter set.
    pub gravity_ticks_by_distance: Vec<u16>,
}

/// Score tables and target-score decay.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ScoringConfig {
    /// Color bonus indexed by the link's distinct color count.
    pub color_bonus: Vec<u16>,
    /// Group bonus indexed by group size; sizes past the tail share it.
    pub group_bonus: Vec<u16>,
    /// Score needed per nuisance ball before any margin decay.
    pub target_points: u64,
    /// Target-score decay over the course of a round.
    pub margin: MarginConfig,
}

/// Margin decay written as an integer table plus its source parameters.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MarginConfig {
    /// Round tick at which the first decay step applies.
    pub start_ticks: u64,
    /// Ticks between subsequent decay steps.
    pub step_ticks: u64,
    /// Authoritative target score per decay step; index zero is the initial value.
    pub target_points_by_step: Vec<u64>,
    /// Where the table came from. Source information, not runtime authority.
    pub source: MarginSource,
}

/// Generation parameters for the margin table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct MarginSource {
    /// Numerator of the per-step ratio.
    pub ratio_numerator: u64,
    /// Denominator of the per-step ratio.
    pub ratio_denominator: u64,
    /// Maximum number of decay steps.
    pub max_steps: u8,
}

/// How score becomes attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct OffenseConfig {
    /// Display points awarded per soft-dropped cell.
    pub soft_drop_points_per_cell: u64,
    /// Whether soft-drop points reach the attack conversion.
    pub soft_drop_counts_toward_attack: bool,
}

/// Exact nuisance queue limits frozen into a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct NuisanceConfig {
    /// Largest pending count a single channel may hold.
    pub queue_limit: u32,
    /// Largest batch one no-chain drop releases.
    pub drop_limit: u32,
}

/// Fever gauge, time and puzzle-level values.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FeverConfig {
    /// Gauge cells needed to enter Fever.
    pub gauge_capacity: u8,
    /// Player-level Fever time at round start.
    pub initial_time_ticks: u32,
    /// Lower clamp for Fever time.
    pub min_time_ticks: u32,
    /// Upper clamp for Fever time.
    pub max_time_ticks: u32,
    /// Lowest selectable puzzle level.
    pub min_level: u8,
    /// Highest selectable puzzle level.
    pub max_level: u8,
    /// Time granted to the attacker whose chain was offset.
    pub offset_reward_ticks: u32,
    /// Time granted by an all clear.
    pub all_clear_reward_ticks: u32,
    /// Puzzle loaded onto the normal board after a normal-board all clear.
    pub all_clear_puzzle_id: String,
    /// Level transitions after a puzzle attempt.
    pub level_ladder: FeverLadderConfig,
}

/// Level deltas applied after a Fever puzzle attempt, before clamping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct FeverLadderConfig {
    /// Delta from the achieved chain when the target was met.
    pub on_target: i8,
    /// Delta from the achieved chain on a Fever all clear.
    pub on_all_clear: i8,
    /// Delta from the achieved chain when short by two.
    pub miss_by_two: i8,
    /// Delta from the achieved chain when short by three or more.
    pub miss_by_more: i8,
}

// ---------------------------------------------------------------------------
// Content library
// ---------------------------------------------------------------------------

/// Character identities, versioned separately from their gameplay data.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Roster {
    /// Document schema version.
    pub schema_version: u32,
    /// Every selectable character.
    pub characters: Vec<CharacterIdentity>,
}

/// A character's identity in the rules core.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CharacterIdentity {
    /// Stable identity referenced by gameplay data and match requests.
    pub id: CharacterId,
    /// Localization key for the display name.
    pub display_name_key: String,
}

impl Digestible for CharacterIdentity {
    fn digest_into(&self, writer: &mut DigestWriter) {
        self.id.digest_into(writer);
        writer.str(&self.display_name_key);
    }
}

impl Digestible for Roster {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u32(self.schema_version);
        writer.seq(&self.characters);
    }
}

/// Per-character gameplay data inside one profile partition.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CharacterPlay {
    /// Document schema version.
    pub schema_version: u32,
    /// Profile partition this data belongs to.
    pub profile_id: RuleProfileId,
    /// Character this data belongs to.
    pub character_id: CharacterId,
    /// Sixteen-hand drop cycle.
    pub drop_set: DropSet,
    /// Chain power tables plus their generation parameters.
    pub chain_power: ChainPowerContent,
}

/// Chain power curves: authoritative integer tables plus their source.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChainPowerContent {
    /// Normal-board table, 24 samples.
    pub normal: Vec<u16>,
    /// Fever-board table, 24 samples.
    pub fever: Vec<u16>,
    /// Parameters the tables were generated from. Source information only.
    pub source: ChainPowerSource,
}

/// Generation parameters stored as scaled integers.
///
/// The rules admission path stays free of floating point; only the offline
/// generator converts these to the real-valued curve family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct ChainPowerSource {
    /// Normal-board ten-chain anchor.
    pub normal_anchor: u16,
    /// Normal-board tilt, in thousandths.
    pub normal_tilt_milli: u16,
    /// Normal-board post-ten growth, in thousandths.
    pub normal_growth_milli: u16,
    /// Fever-board tail base.
    pub fever_anchor: u16,
    /// Fever-board tilt, in thousandths.
    pub fever_tilt_milli: u16,
}

impl ChainPowerSource {
    /// Converts to the offline generator's real-valued parameters.
    #[must_use]
    pub fn parameters(self) -> ChainPowerParameters {
        ChainPowerParameters {
            normal_anchor: f64::from(self.normal_anchor),
            normal_tilt: f64::from(self.normal_tilt_milli) / 1000.0,
            normal_growth: f64::from(self.normal_growth_milli) / 1000.0,
            fever_anchor: f64::from(self.fever_anchor),
            fever_tilt: f64::from(self.fever_tilt_milli) / 1000.0,
        }
    }
}

/// Shape of one hand in a drop set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum DropShape {
    /// Two balls stacked on the pivot.
    I,
    /// Three balls: corner pivot with an upward and a rightward arm.
    L,
    /// Three balls: corner pivot with an upward and a leftward arm.
    J,
    /// Four balls in a 2x2 block using two different colors.
    ODual,
    /// Four balls in a 2x2 block, single color; rotation cycles the color.
    OMono,
}

impl DropShape {
    /// Number of balls this shape supplies.
    #[must_use]
    pub const fn ball_count(self) -> u8 {
        match self {
            Self::I => 2,
            Self::L | Self::J => 3,
            Self::ODual | Self::OMono => 4,
        }
    }

    /// Whether the color layout is a per-hand choice rather than fixed.
    #[must_use]
    pub const fn needs_layout(self) -> bool {
        matches!(self, Self::L | Self::J)
    }

    /// Mirror used when the L/J cycle swaps this hand.
    #[must_use]
    pub const fn swapped(self) -> Self {
        match self {
            Self::L => Self::J,
            Self::J => Self::L,
            other => other,
        }
    }
}

impl Digestible for DropShape {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u8(match self {
            Self::I => 0,
            Self::L => 1,
            Self::J => 2,
            Self::ODual => 3,
            Self::OMono => 4,
        });
    }
}

/// Which of a hand's two drawn colors a ball takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSlot {
    /// The hand's first drawn color.
    First,
    /// The hand's second drawn color.
    Second,
}

/// One ball of a hand, expressed around the spawn pivot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropBallTemplate {
    /// Column offset from the pivot.
    pub dx: i8,
    /// Row offset from the pivot; negative is upward.
    pub dy: i8,
    /// Which drawn color this ball takes.
    pub color_slot: ColorSlot,
}

/// One hand of a drop set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct DropTemplate {
    /// The hand's shape.
    pub shape: DropShape,
    /// For `L`/`J`, whether the vertical pair takes the first color.
    #[serde(default)]
    pub vertical_pair_first: Option<bool>,
}

impl DropTemplate {
    /// Spawn-pose balls with their color slots, in canonical order.
    #[must_use]
    pub fn balls(&self) -> Vec<DropBallTemplate> {
        let ball = |dx, dy, color_slot| DropBallTemplate { dx, dy, color_slot };
        match self.shape {
            DropShape::I => vec![ball(0, 0, ColorSlot::First), ball(0, -1, ColorSlot::Second)],
            DropShape::L | DropShape::J => {
                let arm_x = if matches!(self.shape, DropShape::L) {
                    1
                } else {
                    -1
                };
                // `None` cannot survive validation; treat it as the vertical
                // layout so this stays total.
                let vertical_first = self.vertical_pair_first.unwrap_or(true);
                let (vertical, horizontal) = if vertical_first {
                    (ColorSlot::First, ColorSlot::Second)
                } else {
                    (ColorSlot::Second, ColorSlot::First)
                };
                vec![
                    ball(0, 0, vertical),
                    ball(0, -1, vertical),
                    ball(arm_x, 0, horizontal),
                ]
            }
            DropShape::ODual => vec![
                ball(0, 0, ColorSlot::First),
                ball(1, 0, ColorSlot::First),
                ball(0, -1, ColorSlot::Second),
                ball(1, -1, ColorSlot::Second),
            ],
            DropShape::OMono => vec![
                ball(0, 0, ColorSlot::First),
                ball(1, 0, ColorSlot::First),
                ball(0, -1, ColorSlot::First),
                ball(1, -1, ColorSlot::First),
            ],
        }
    }

    /// How many colors this hand draws, and whether they must differ.
    #[must_use]
    pub const fn color_draw(&self) -> ColorDraw {
        match self.shape {
            DropShape::OMono => ColorDraw::Single,
            DropShape::ODual => ColorDraw::Distinct,
            DropShape::I | DropShape::L | DropShape::J => ColorDraw::Independent,
        }
    }
}

/// How a hand draws its colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDraw {
    /// One color for the whole hand.
    Single,
    /// Two independent draws; they may coincide.
    Independent,
    /// Two draws that must differ, as a dual-color `O` needs two rows.
    Distinct,
}

impl Digestible for DropTemplate {
    fn digest_into(&self, writer: &mut DigestWriter) {
        self.shape.digest_into(writer);
        match self.vertical_pair_first {
            Some(value) => {
                writer.u8(1);
                writer.bool(value);
            }
            None => writer.u8(0),
        }
    }
}

/// A character's sixteen-hand cycle.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DropSet(pub Vec<DropTemplate>);

impl DropSet {
    /// Number of single-color four-ball hands in the cycle.
    ///
    /// The L/J cycle is derived from this count's parity rather than being
    /// configured separately.
    #[must_use]
    pub fn mono_quad_count(&self) -> usize {
        self.0
            .iter()
            .filter(|hand| matches!(hand.shape, DropShape::OMono))
            .count()
    }

    /// Whether L and J swap in every other sixteen-hand cycle.
    #[must_use]
    pub fn swaps_l_and_j(&self) -> bool {
        self.mono_quad_count() % 2 == 1
    }

    /// Total balls supplied by one cycle.
    #[must_use]
    pub fn total_balls(&self) -> u32 {
        self.0
            .iter()
            .map(|hand| u32::from(hand.shape.ball_count()))
            .sum()
    }

    /// The hand at `cursor`, with the L/J cycle already applied.
    #[must_use]
    pub fn hand(&self, cursor: u64) -> DropTemplate {
        let index = (cursor as usize) % self.0.len();
        let cycle = (cursor as usize) / self.0.len();
        let mut hand = self.0[index];
        if self.swaps_l_and_j() && cycle % 2 == 1 {
            hand.shape = hand.shape.swapped();
        }
        hand
    }
}

impl Digestible for DropSet {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.seq(&self.0);
    }
}

impl Digestible for ChainPowerSource {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u16(self.normal_anchor);
        writer.u16(self.normal_tilt_milli);
        writer.u16(self.normal_growth_milli);
        writer.u16(self.fever_anchor);
        writer.u16(self.fever_tilt_milli);
    }
}

impl Digestible for ChainPowerContent {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.seq(&self.normal);
        writer.seq(&self.fever);
        self.source.digest_into(writer);
    }
}

impl Digestible for CharacterPlay {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u32(self.schema_version);
        self.profile_id.digest_into(writer);
        self.character_id.digest_into(writer);
        self.drop_set.digest_into(writer);
        self.chain_power.digest_into(writer);
    }
}

impl CharacterPlay {
    /// Converts the stored samples to runtime-authoritative curves.
    pub fn chain_power_profile(&self) -> Result<ChainPowerProfile, ConfigError> {
        let table =
            |name: &str, values: &[u16]| -> Result<[u16; CHAIN_POWER_TABLE_LEN], ConfigError> {
                values.to_vec().try_into().map_err(|values: Vec<u16>| {
                    integrity(
                        format!("play.{}.chain_power.{name}", self.character_id.0),
                        format!(
                            "expected {CHAIN_POWER_TABLE_LEN} samples, found {}",
                            values.len()
                        ),
                    )
                })
            };
        let normal = table("normal", &self.chain_power.normal)?;
        let fever = table("fever", &self.chain_power.fever)?;
        ChainPowerProfile::new(normal, fever).map_err(|error| {
            integrity(
                format!("play.{}.chain_power", self.character_id.0),
                error.to_string(),
            )
        })
    }
}

/// A Fever puzzle book for one profile partition.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FeverPuzzleBook {
    /// Document schema version.
    pub schema_version: u32,
    /// Profile partition this book belongs to.
    pub profile_id: RuleProfileId,
    /// Every puzzle, across all levels.
    pub puzzles: Vec<FeverPuzzle>,
}

/// One Fever puzzle layout.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FeverPuzzle {
    /// Stable puzzle identity.
    pub id: String,
    /// Target chain level this puzzle belongs to.
    pub level: u8,
    /// Occupied cells with their colors.
    pub cells: Vec<PuzzleCell>,
}

/// One occupied cell of a Fever puzzle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct PuzzleCell {
    /// Column.
    pub x: u8,
    /// Row, counted from the top of the board.
    pub y: u8,
    /// Color id.
    pub color: u8,
}

impl Digestible for PuzzleCell {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u8(self.x);
        writer.u8(self.y);
        writer.u8(self.color);
    }
}

impl Digestible for FeverPuzzle {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.str(&self.id);
        writer.u8(self.level);
        writer.seq(&self.cells);
    }
}

impl Digestible for FeverPuzzleBook {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u32(self.schema_version);
        self.profile_id.digest_into(writer);
        writer.seq(&self.puzzles);
    }
}

impl Digestible for FieldConfig {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u8(self.width);
        writer.u8(self.height);
        writer.u8(self.hidden_rows);
        writer.u8(self.spawn_column);
        writer.u8(self.color_count);
    }
}

impl Digestible for DropConfig {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u8(self.next_queue_len);
        writer.u16(self.natural_fall_ticks);
        writer.u16(self.soft_drop_ticks);
        writer.u16(self.horizontal_repeat_delay_ticks);
        writer.u16(self.horizontal_repeat_interval_ticks);
        writer.u16(self.horizontal_cooldown_ticks);
        writer.u16(self.lock_delay_ticks);
        writer.u8(self.lift_limit);
        writer.u16(self.split_delay_pivot_ticks);
        writer.u16(self.split_delay_follower_ticks);
    }
}

impl Digestible for RotationConfig {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u16(self.cooldown_ticks);
        writer.u8(self.double_rotation_period);
    }
}

impl Digestible for ResolveConfig {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u8(self.clear_threshold);
        writer.u16(self.clear_preview_ticks);
        writer.seq(&self.gravity_ticks_by_distance);
    }
}

impl Digestible for MarginSource {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u64(self.ratio_numerator);
        writer.u64(self.ratio_denominator);
        writer.u8(self.max_steps);
    }
}

impl Digestible for MarginConfig {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u64(self.start_ticks);
        writer.u64(self.step_ticks);
        writer.seq(&self.target_points_by_step);
        self.source.digest_into(writer);
    }
}

impl Digestible for ScoringConfig {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.seq(&self.color_bonus);
        writer.seq(&self.group_bonus);
        writer.u64(self.target_points);
        self.margin.digest_into(writer);
    }
}

impl Digestible for OffenseConfig {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u64(self.soft_drop_points_per_cell);
        writer.bool(self.soft_drop_counts_toward_attack);
    }
}

impl Digestible for NuisanceConfig {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u32(self.queue_limit);
        writer.u32(self.drop_limit);
    }
}

impl Digestible for FeverLadderConfig {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.i8(self.on_target);
        writer.i8(self.on_all_clear);
        writer.i8(self.miss_by_two);
        writer.i8(self.miss_by_more);
    }
}

impl Digestible for FeverConfig {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u8(self.gauge_capacity);
        writer.u32(self.initial_time_ticks);
        writer.u32(self.min_time_ticks);
        writer.u32(self.max_time_ticks);
        writer.u8(self.min_level);
        writer.u8(self.max_level);
        writer.u32(self.offset_reward_ticks);
        writer.u32(self.all_clear_reward_ticks);
        writer.str(&self.all_clear_puzzle_id);
        self.level_ladder.digest_into(writer);
    }
}

impl Digestible for RuleProfile {
    fn digest_into(&self, writer: &mut DigestWriter) {
        writer.u32(self.schema_version);
        self.id.digest_into(writer);
        writer.str(&self.rule_version);
        writer.str(&self.reference_profile);
        self.field.digest_into(writer);
        self.drop.digest_into(writer);
        self.rotation.digest_into(writer);
        self.resolve.digest_into(writer);
        self.scoring.digest_into(writer);
        self.offense.digest_into(writer);
        self.nuisance.digest_into(writer);
        self.fever.digest_into(writer);
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn from_ron<T: serde::de::DeserializeOwned>(source: &str) -> Result<T, ConfigError> {
    ron::from_str(source).map_err(|error| ConfigError::Ron(error.to_string()))
}

fn check_schema(found: u32) -> Result<(), ConfigError> {
    if found == RULE_PROFILE_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ConfigError::UnsupportedSchema {
            found,
            supported: RULE_PROFILE_SCHEMA_VERSION,
        })
    }
}

/// Parses one versioned rule profile from in-memory RON.
pub fn parse_rule_profile(source: &str) -> Result<RuleProfile, ConfigError> {
    let profile: RuleProfile = from_ron(source)?;
    check_schema(profile.schema_version)?;
    validate_profile_integrity(&profile)?;
    validate_profile_consistency(&profile)?;
    Ok(profile)
}

/// Parses the versioned character roster from in-memory RON.
pub fn parse_roster(source: &str) -> Result<Roster, ConfigError> {
    let roster: Roster = from_ron(source)?;
    check_schema(roster.schema_version)?;
    validate_roster(&roster)?;
    Ok(roster)
}

/// Parses one versioned character gameplay document from in-memory RON.
pub fn parse_character_play(source: &str) -> Result<CharacterPlay, ConfigError> {
    let play: CharacterPlay = from_ron(source)?;
    check_schema(play.schema_version)?;
    validate_play_integrity(&play)?;
    Ok(play)
}

/// Parses one versioned Fever puzzle book from in-memory RON.
pub fn parse_fever_puzzle_book(source: &str) -> Result<FeverPuzzleBook, ConfigError> {
    let book: FeverPuzzleBook = from_ron(source)?;
    check_schema(book.schema_version)?;
    Ok(book)
}

// ---------------------------------------------------------------------------
// Layer 1: general integrity
// ---------------------------------------------------------------------------

fn validate_profile_integrity(profile: &RuleProfile) -> Result<(), ConfigError> {
    if profile.id.0.is_empty() {
        return Err(integrity("profile.id", "must not be empty"));
    }
    if profile.rule_version.is_empty() {
        return Err(integrity("profile.rule_version", "must not be empty"));
    }
    if profile.reference_profile.is_empty() {
        return Err(integrity("profile.reference_profile", "must not be empty"));
    }

    let field = &profile.field;
    if field.geometry().is_none() {
        return Err(integrity(
            "profile.field",
            "width, height, hidden_rows and spawn_column must form a valid board",
        ));
    }
    if field.color_count < 2 {
        return Err(integrity("profile.field.color_count", "must be at least 2"));
    }

    let drop = &profile.drop;
    for (path, ticks) in [
        ("profile.drop.natural_fall_ticks", drop.natural_fall_ticks),
        ("profile.drop.soft_drop_ticks", drop.soft_drop_ticks),
        (
            "profile.drop.horizontal_repeat_delay_ticks",
            drop.horizontal_repeat_delay_ticks,
        ),
        (
            "profile.drop.horizontal_repeat_interval_ticks",
            drop.horizontal_repeat_interval_ticks,
        ),
        (
            "profile.drop.horizontal_cooldown_ticks",
            drop.horizontal_cooldown_ticks,
        ),
        ("profile.drop.lock_delay_ticks", drop.lock_delay_ticks),
        (
            "profile.drop.split_delay_pivot_ticks",
            drop.split_delay_pivot_ticks,
        ),
        (
            "profile.drop.split_delay_follower_ticks",
            drop.split_delay_follower_ticks,
        ),
    ] {
        if ticks == 0 {
            return Err(integrity(path, "tick duration must be positive"));
        }
    }
    if drop.next_queue_len == 0 {
        return Err(integrity("profile.drop.next_queue_len", "must be positive"));
    }
    if drop.lift_limit == 0 {
        return Err(integrity("profile.drop.lift_limit", "must be positive"));
    }

    if profile.rotation.cooldown_ticks == 0 {
        return Err(integrity(
            "profile.rotation.cooldown_ticks",
            "tick duration must be positive",
        ));
    }
    if profile.rotation.double_rotation_period < 2 {
        return Err(integrity(
            "profile.rotation.double_rotation_period",
            "must be at least 2",
        ));
    }

    let resolve = &profile.resolve;
    if resolve.clear_threshold < 2 {
        return Err(integrity(
            "profile.resolve.clear_threshold",
            "must be at least 2",
        ));
    }
    if resolve.clear_preview_ticks == 0 {
        return Err(integrity(
            "profile.resolve.clear_preview_ticks",
            "tick duration must be positive",
        ));
    }
    if resolve.gravity_ticks_by_distance.len() < 2 {
        return Err(integrity(
            "profile.resolve.gravity_ticks_by_distance",
            "needs a zero-distance entry and at least one fall distance",
        ));
    }
    if resolve.gravity_ticks_by_distance[0] != 0 {
        return Err(integrity(
            "profile.resolve.gravity_ticks_by_distance[0]",
            "zero distance must take zero ticks",
        ));
    }
    if resolve.gravity_ticks_by_distance[1..].contains(&0) {
        return Err(integrity(
            "profile.resolve.gravity_ticks_by_distance",
            "a non-zero fall distance must take positive ticks",
        ));
    }
    if resolve.gravity_ticks_by_distance.len() <= usize::from(field.height - field.hidden_rows) {
        return Err(integrity(
            "profile.resolve.gravity_ticks_by_distance",
            "must cover every reachable fall distance",
        ));
    }

    let scoring = &profile.scoring;
    if scoring.color_bonus.len() <= usize::from(field.color_count) {
        return Err(integrity(
            "profile.scoring.color_bonus",
            "must have an entry for every reachable color count",
        ));
    }
    if scoring.group_bonus.len() <= usize::from(resolve.clear_threshold) {
        return Err(integrity(
            "profile.scoring.group_bonus",
            "must have an entry for every clearing group size",
        ));
    }
    if scoring.target_points == 0 {
        return Err(integrity(
            "profile.scoring.target_points",
            "must be positive",
        ));
    }
    if scoring.margin.step_ticks == 0 {
        return Err(integrity(
            "profile.scoring.margin.step_ticks",
            "must be positive",
        ));
    }
    if scoring
        .margin
        .target_points_by_step
        .first()
        .is_none_or(|first| *first != scoring.target_points)
    {
        return Err(integrity(
            "profile.scoring.margin.target_points_by_step[0]",
            "must equal profile.scoring.target_points",
        ));
    }
    if scoring.margin.target_points_by_step.contains(&0) {
        return Err(integrity(
            "profile.scoring.margin.target_points_by_step",
            "target points must stay positive",
        ));
    }
    if scoring
        .margin
        .target_points_by_step
        .windows(2)
        .any(|pair| pair[0] < pair[1])
    {
        return Err(integrity(
            "profile.scoring.margin.target_points_by_step",
            "target points must not increase",
        ));
    }

    if profile.nuisance.queue_limit == 0 {
        return Err(integrity(
            "profile.nuisance.queue_limit",
            "must be positive",
        ));
    }
    if profile.nuisance.drop_limit == 0 {
        return Err(integrity("profile.nuisance.drop_limit", "must be positive"));
    }
    if profile.nuisance.drop_limit > profile.nuisance.queue_limit {
        return Err(integrity(
            "profile.nuisance.drop_limit",
            "must not exceed profile.nuisance.queue_limit",
        ));
    }

    let fever = &profile.fever;
    if fever.min_time_ticks > fever.initial_time_ticks
        || fever.initial_time_ticks > fever.max_time_ticks
    {
        return Err(integrity(
            "profile.fever",
            "requires min_time_ticks <= initial_time_ticks <= max_time_ticks",
        ));
    }
    if fever.min_level > fever.max_level {
        return Err(integrity(
            "profile.fever.min_level",
            "must not exceed profile.fever.max_level",
        ));
    }
    if fever.min_level == 0 {
        return Err(integrity("profile.fever.min_level", "must be at least 1"));
    }
    if fever.all_clear_puzzle_id.is_empty() {
        return Err(integrity(
            "profile.fever.all_clear_puzzle_id",
            "must not be empty",
        ));
    }
    Ok(())
}

fn validate_roster(roster: &Roster) -> Result<(), ConfigError> {
    if roster.characters.is_empty() {
        return Err(integrity("roster.characters", "must not be empty"));
    }
    let mut seen = BTreeSet::new();
    for character in &roster.characters {
        if character.id.0.is_empty() {
            return Err(integrity("roster.characters[].id", "must not be empty"));
        }
        if !seen.insert(character.id.clone()) {
            return Err(integrity(
                format!("roster.characters[{}]", character.id.0),
                "character ids must be unique",
            ));
        }
    }
    Ok(())
}

fn validate_play_integrity(play: &CharacterPlay) -> Result<(), ConfigError> {
    let path = format!("play.{}", play.character_id.0);
    if play.character_id.0.is_empty() {
        return Err(integrity(path, "character_id must not be empty"));
    }
    if play.drop_set.0.len() != DROP_SET_LEN {
        return Err(integrity(
            format!("{path}.drop_set"),
            format!(
                "must contain {DROP_SET_LEN} hands, found {}",
                play.drop_set.0.len()
            ),
        ));
    }
    for (index, hand) in play.drop_set.0.iter().enumerate() {
        match (hand.shape.needs_layout(), hand.vertical_pair_first) {
            (true, None) => {
                return Err(integrity(
                    format!("{path}.drop_set[{index}].vertical_pair_first"),
                    "L and J hands must declare their color layout",
                ));
            }
            (false, Some(_)) => {
                return Err(integrity(
                    format!("{path}.drop_set[{index}].vertical_pair_first"),
                    "only L and J hands take a color layout",
                ));
            }
            _ => {}
        }
    }
    play.chain_power_profile()?;
    Ok(())
}

fn validate_puzzle_integrity(
    book: &FeverPuzzleBook,
    profile: &RuleProfile,
) -> Result<(), ConfigError> {
    let field = &profile.field;
    let mut seen = BTreeSet::new();
    for puzzle in &book.puzzles {
        let path = format!("puzzles.{}", puzzle.id);
        if puzzle.id.is_empty() {
            return Err(integrity("puzzles[].id", "must not be empty"));
        }
        if !seen.insert(puzzle.id.clone()) {
            return Err(integrity(path, "puzzle ids must be unique"));
        }
        if puzzle.level < profile.fever.min_level || puzzle.level > profile.fever.max_level {
            return Err(integrity(
                format!("{path}.level"),
                format!(
                    "level {} is outside the declared domain {}..={}",
                    puzzle.level, profile.fever.min_level, profile.fever.max_level
                ),
            ));
        }
        let mut occupied = BTreeSet::new();
        for cell in &puzzle.cells {
            if cell.x >= field.width || cell.y >= field.height {
                return Err(integrity(
                    format!("{path}.cells"),
                    format!(
                        "cell ({}, {}) is outside the {}x{} board",
                        cell.x, cell.y, field.width, field.height
                    ),
                ));
            }
            if cell.y < field.hidden_rows {
                return Err(integrity(
                    format!("{path}.cells"),
                    format!("cell ({}, {}) sits in a hidden row", cell.x, cell.y),
                ));
            }
            if cell.color >= field.color_count {
                return Err(integrity(
                    format!("{path}.cells"),
                    format!("color {} is outside the declared color count", cell.color),
                ));
            }
            if !occupied.insert((cell.x, cell.y)) {
                return Err(integrity(
                    format!("{path}.cells"),
                    format!("cell ({}, {}) is occupied twice", cell.x, cell.y),
                ));
            }
        }
        // A puzzle is loaded onto a settled board, so it must already be
        // gravity-stable: nothing may float above an empty cell.
        for (x, y) in &occupied {
            if *y + 1 < field.height && !occupied.contains(&(*x, y + 1)) {
                return Err(integrity(
                    format!("{path}.cells"),
                    format!("cell ({x}, {y}) floats above an empty cell"),
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Layer 2: profile self-consistency
// ---------------------------------------------------------------------------

fn validate_profile_consistency(profile: &RuleProfile) -> Result<(), ConfigError> {
    let fever = &profile.fever;
    if fever.gauge_capacity == 0 {
        return Err(consistency(
            "profile.fever.gauge_capacity",
            "a gauge that never fills cannot enter Fever",
        ));
    }
    let domain_width = i32::from(fever.max_level) - i32::from(fever.min_level);
    for (path, delta) in [
        ("on_target", fever.level_ladder.on_target),
        ("on_all_clear", fever.level_ladder.on_all_clear),
        ("miss_by_two", fever.level_ladder.miss_by_two),
        ("miss_by_more", fever.level_ladder.miss_by_more),
    ] {
        if i32::from(delta).abs() > domain_width {
            return Err(consistency(
                format!("profile.fever.level_ladder.{path}"),
                format!(
                    "a delta of {delta} cannot stay inside the level domain {}..={}",
                    fever.min_level, fever.max_level
                ),
            ));
        }
    }

    // Bound the whole scoring-to-nuisance path at its worst case. The score
    // multiplier is clamped, so the tables cannot overflow it; what can
    // overflow is the pending-nuisance width once a maximal round score is
    // divided by the smallest target score the margin table reaches.
    let scoring = &profile.scoring;
    let cells = u64::from(profile.field.visible_cells());
    let max_link_score = 10 * cells * u64::from(crate::rules::CHAIN_POWER_MAX)
        + u64::from(scoring.color_bonus.iter().copied().max().unwrap_or(0));
    let max_round_score = max_link_score
        .checked_mul(CHAIN_POWER_TABLE_LEN as u64)
        .ok_or_else(|| {
            consistency(
                "profile.scoring",
                "worst-case round score overflows the scoring accumulator",
            )
        })?;
    let min_target = scoring
        .margin
        .target_points_by_step
        .iter()
        .copied()
        .min()
        .unwrap_or(scoring.target_points)
        .max(1);
    let max_attack = max_round_score / min_target;
    let worst_pending = max_attack
        .checked_add(u64::from(profile.nuisance.queue_limit))
        .filter(|pending| u32::try_from(*pending).is_ok())
        .ok_or_else(|| {
            consistency(
                "profile.nuisance.queue_limit",
                format!(
                    "queue limit {} plus a worst-case attack of {max_attack} exceeds the pending-nuisance width",
                    profile.nuisance.queue_limit
                ),
            )
        })?;
    debug_assert!(u32::try_from(worst_pending).is_ok());
    Ok(())
}

// ---------------------------------------------------------------------------
// Layer 3: content coverage
// ---------------------------------------------------------------------------

/// A character the content library cannot make selectable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableCharacter {
    /// The character that cannot be selected.
    pub character: CharacterId,
    /// Why it cannot be selected.
    pub reason: ConfigError,
}

/// What a partial library build had to exclude.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryReport {
    /// Characters excluded from selection, with their reasons.
    pub unavailable_characters: Vec<UnavailableCharacter>,
}

/// Validated in-memory content; callers cannot freeze a match from raw files.
#[derive(Debug, Clone)]
pub struct ValidatedRuleLibrary {
    profiles: BTreeMap<RuleProfileId, RuleProfile>,
    profile_digests: BTreeMap<RuleProfileId, ContentDigest>,
    roster: Roster,
    roster_digest: ContentDigest,
    plays: BTreeMap<(RuleProfileId, CharacterId), CharacterPlay>,
    play_digests: BTreeMap<(RuleProfileId, CharacterId), ContentDigest>,
    books: BTreeMap<RuleProfileId, FeverPuzzleBook>,
    book_digests: BTreeMap<RuleProfileId, ContentDigest>,
    root: ContentDigest,
}

impl ValidatedRuleLibrary {
    /// Validates a complete content set: every rostered character must be playable.
    pub fn new(
        profiles: Vec<RuleProfile>,
        roster: Roster,
        plays: Vec<CharacterPlay>,
        books: Vec<FeverPuzzleBook>,
    ) -> Result<Self, ConfigError> {
        let (library, report) = Self::partial(profiles, roster, plays, books)?;
        if let Some(first) = report.unavailable_characters.first() {
            return Err(first.reason.clone());
        }
        Ok(library)
    }

    /// Validates a content set, excluding characters whose data is unusable.
    ///
    /// A missing profile or puzzle book still fails outright: without them no
    /// match is possible at all. A single character's gameplay data only
    /// removes that character from selection.
    pub fn partial(
        profiles: Vec<RuleProfile>,
        roster: Roster,
        plays: Vec<CharacterPlay>,
        books: Vec<FeverPuzzleBook>,
    ) -> Result<(Self, LibraryReport), ConfigError> {
        validate_roster(&roster)?;

        let mut profile_map = BTreeMap::new();
        let mut profile_digests = BTreeMap::new();
        for profile in profiles {
            validate_profile_integrity(&profile)?;
            validate_profile_consistency(&profile)?;
            profile_digests.insert(profile.id.clone(), profile.content_digest());
            if profile_map.insert(profile.id.clone(), profile).is_some() {
                return Err(integrity("profile.id", "rule profile ids must be unique"));
            }
        }

        let mut book_map = BTreeMap::new();
        let mut book_digests = BTreeMap::new();
        for book in books {
            let profile = profile_map.get(&book.profile_id).ok_or_else(|| {
                integrity(
                    format!("puzzle_book.profile_id[{}]", book.profile_id.0),
                    "references an unknown rule profile",
                )
            })?;
            validate_puzzle_integrity(&book, profile)?;
            book_digests.insert(book.profile_id.clone(), book.content_digest());
            if book_map.insert(book.profile_id.clone(), book).is_some() {
                return Err(integrity(
                    "puzzle_book.profile_id",
                    "a profile takes at most one puzzle book",
                ));
            }
        }

        // Every profile needs a book that covers its whole declared level
        // domain, plus the all-clear puzzle it names.
        for (id, profile) in &profile_map {
            let book = book_map.get(id).ok_or_else(|| {
                coverage(
                    format!("puzzle_book[{}]", id.0),
                    "the profile declares Fever levels but has no puzzle book",
                )
            })?;
            for level in profile.fever.min_level..=profile.fever.max_level {
                if !book.puzzles.iter().any(|puzzle| puzzle.level == level) {
                    return Err(coverage(
                        format!("puzzle_book[{}].puzzles", id.0),
                        format!("no puzzle covers declared level {level}"),
                    ));
                }
            }
            if !book
                .puzzles
                .iter()
                .any(|puzzle| puzzle.id == profile.fever.all_clear_puzzle_id)
            {
                return Err(coverage(
                    format!("profile[{}].fever.all_clear_puzzle_id", id.0),
                    format!(
                        "puzzle {} is not in the profile's puzzle book",
                        profile.fever.all_clear_puzzle_id
                    ),
                ));
            }
        }

        let mut play_map = BTreeMap::new();
        let mut play_digests = BTreeMap::new();
        let mut report = LibraryReport::default();
        let rostered: BTreeSet<_> = roster
            .characters
            .iter()
            .map(|character| character.id.clone())
            .collect();
        for play in plays {
            if !profile_map.contains_key(&play.profile_id) {
                return Err(integrity(
                    format!("play[{}].profile_id", play.character_id.0),
                    format!("references unknown profile {}", play.profile_id.0),
                ));
            }
            if !rostered.contains(&play.character_id) {
                return Err(integrity(
                    format!("play[{}].character_id", play.character_id.0),
                    "references a character that is not on the roster",
                ));
            }
            if let Err(reason) = validate_play_integrity(&play) {
                report.unavailable_characters.push(UnavailableCharacter {
                    character: play.character_id.clone(),
                    reason,
                });
                continue;
            }
            let key = (play.profile_id.clone(), play.character_id.clone());
            play_digests.insert(key.clone(), play.content_digest());
            if play_map.insert(key, play).is_some() {
                return Err(integrity(
                    "play.character_id",
                    "a character takes at most one gameplay document per profile",
                ));
            }
        }

        // Coverage: a rostered character with no gameplay data under a profile
        // is unselectable under that profile, not a whole-library failure.
        for profile_id in profile_map.keys() {
            for character in &rostered {
                let key = (profile_id.clone(), character.clone());
                if !play_map.contains_key(&key)
                    && !report
                        .unavailable_characters
                        .iter()
                        .any(|entry| entry.character == *character)
                {
                    report.unavailable_characters.push(UnavailableCharacter {
                        character: character.clone(),
                        reason: coverage(
                            format!("play[{}][{}]", profile_id.0, character.0),
                            "the profile declares gameplay data that this character does not provide",
                        ),
                    });
                }
            }
        }

        let roster_digest = roster.content_digest();
        let mut subjects = vec![roster_digest];
        subjects.extend(profile_digests.values().copied());
        subjects.extend(book_digests.values().copied());
        subjects.extend(play_digests.values().copied());
        let root = root_digest(&subjects);

        Ok((
            Self {
                profiles: profile_map,
                profile_digests,
                roster,
                roster_digest,
                plays: play_map,
                play_digests,
                books: book_map,
                book_digests,
                root,
            },
            report,
        ))
    }

    /// Returns a profile selected by id.
    #[must_use]
    pub fn profile(&self, id: &RuleProfileId) -> Option<&RuleProfile> {
        self.profiles.get(id)
    }

    /// Returns the character roster.
    #[must_use]
    pub const fn roster(&self) -> &Roster {
        &self.roster
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

    /// Returns the Fever puzzle book for a profile.
    #[must_use]
    pub fn puzzle_book(&self, profile: &RuleProfileId) -> Option<&FeverPuzzleBook> {
        self.books.get(profile)
    }

    /// Root digest over every subject in this library.
    #[must_use]
    pub const fn root_digest(&self) -> ContentDigest {
        self.root
    }

    /// Digest of the roster subject.
    #[must_use]
    pub const fn roster_digest(&self) -> ContentDigest {
        self.roster_digest
    }

    /// Digest of one profile subject.
    #[must_use]
    pub fn profile_digest(&self, id: &RuleProfileId) -> Option<ContentDigest> {
        self.profile_digests.get(id).copied()
    }

    /// Digest of one puzzle book subject.
    #[must_use]
    pub fn puzzle_book_digest(&self, id: &RuleProfileId) -> Option<ContentDigest> {
        self.book_digests.get(id).copied()
    }

    /// Digest of one character gameplay subject.
    #[must_use]
    pub fn play_digest(
        &self,
        profile: &RuleProfileId,
        character: &CharacterId,
    ) -> Option<ContentDigest> {
        self.play_digests
            .get(&(profile.clone(), character.clone()))
            .copied()
    }
}

// ---------------------------------------------------------------------------
// Localization stub
// ---------------------------------------------------------------------------

/// Schema version supported by the current stub loaders.
pub const STUB_SCHEMA_VERSION: u32 = 1;

/// Minimal rules document used to prove the data loading path.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RulesStub {
    /// Document schema version.
    pub schema_version: u32,
    /// Stub identity.
    pub id: String,
}

/// Minimal localization table used to prove the i18n loading path.
///
/// Locale-set semantics belong to `client::i18n`; this stub only proves that a
/// versioned JSON document parses into typed data from an in-memory source.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct I18nStub {
    /// Document schema version.
    pub schema_version: u32,
    /// Key/value message table.
    pub messages: BTreeMap<String, String>,
}

/// Parse a rules stub from a RON document string.
pub fn parse_rules_stub(ron_source: &str) -> Result<RulesStub, ConfigError> {
    let stub: RulesStub = from_ron(ron_source)?;
    ensure_stub_schema(stub.schema_version)?;
    Ok(stub)
}

/// Parse an i18n stub from a JSON document string.
pub fn parse_i18n_stub(json_source: &str) -> Result<I18nStub, ConfigError> {
    let stub: I18nStub =
        serde_json::from_str(json_source).map_err(|err| ConfigError::Json(err.to_string()))?;
    ensure_stub_schema(stub.schema_version)?;
    Ok(stub)
}

fn ensure_stub_schema(found: u32) -> Result<(), ConfigError> {
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

    const I18N_EN: &str = include_str!("../../../assets/i18n/en.json");
    const I18N_ZH: &str = include_str!("../../../assets/i18n/zh-CN.json");

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
