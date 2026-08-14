//! Immutable rule projection selected before a BO3 begins.

use crate::{
    board::BoardGeometry,
    config::{
        CharacterId, ConfigError, DropSet, FeverLadderConfig, FeverPuzzleBook, RuleProfile,
        RuleProfileId, ValidatedRuleLibrary,
    },
    determinism::{RNG_ALGORITHM_VERSION, STATE_CODEC_VERSION},
    digest::{ContentDigest, DIGEST_ALGORITHM_VERSION},
    nuisance::NuisanceRules,
    resolution::ResolutionRules,
    rules::ChainPowerProfile,
    scoring::ScoringRules,
};

/// The two participant slots supported by one R1 match.
pub const PARTICIPANT_SLOTS: usize = 2;

/// Request made by the client after both character selections are confirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchRequest {
    /// Which rule profile governs the match.
    pub rule_profile_id: RuleProfileId,
    /// Seed every named random stream derives from.
    pub root_seed: u64,
    /// One character per participant slot.
    pub characters: [CharacterId; PARTICIPANT_SLOTS],
}

/// Versions of the algorithms whose output must match for two runs to compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlgorithmVersions {
    /// Content digest encoding and hash.
    pub digest: u32,
    /// Random stream derivation.
    pub rng: u32,
    /// Canonical state encoding behind the checksum.
    pub state_codec: u32,
}

impl AlgorithmVersions {
    /// Versions this build produces.
    #[must_use]
    pub const fn current() -> Self {
        Self {
            digest: DIGEST_ALGORITHM_VERSION,
            rng: RNG_ALGORITHM_VERSION,
            state_codec: STATE_CODEC_VERSION,
        }
    }
}

/// The digest tree entries this match actually used.
///
/// The root covers every subject in the library; the per-subject digests make
/// a mismatch locatable without re-reading any asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchDigests {
    /// Digest over every subject of the source library.
    pub root: ContentDigest,
    /// Roster subject.
    pub roster: ContentDigest,
    /// Selected rule profile subject.
    pub profile: ContentDigest,
    /// Fever puzzle book subject.
    pub puzzle_book: ContentDigest,
    /// Gameplay data subject per participant slot.
    pub plays: [ContentDigest; PARTICIPANT_SLOTS],
}

/// Falling-group timing frozen for the match, all in ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropTiming {
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
    /// Minimum ticks between two rotations in the same direction.
    pub rotation_cooldown_ticks: u16,
    /// How many blocked attempts release a 180 degree flip.
    pub double_rotation_period: u8,
}

/// Target-score decay frozen for the match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarginRules {
    /// Round tick at which the first decay step applies.
    pub start_ticks: u64,
    /// Ticks between subsequent decay steps.
    pub step_ticks: u64,
    /// Authoritative target score per decay step.
    pub target_points_by_step: Vec<u64>,
}

impl MarginRules {
    /// Decay step index reached at `round_tick`.
    ///
    /// Integer arithmetic only: the runtime looks the target score up, it never
    /// recomputes the decay.
    #[must_use]
    pub fn step_at(&self, round_tick: u64) -> usize {
        if round_tick < self.start_ticks {
            return 0;
        }
        let elapsed = round_tick - self.start_ticks;
        let step = 1 + (elapsed / self.step_ticks) as usize;
        step.min(self.target_points_by_step.len().saturating_sub(1))
    }

    /// Target score in force at `round_tick`.
    #[must_use]
    pub fn target_points_at(&self, round_tick: u64) -> u64 {
        self.target_points_by_step[self.step_at(round_tick)]
    }
}

/// Score-to-attack conversion frozen for the match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffenseRules {
    /// Display points awarded per soft-dropped cell.
    pub soft_drop_points_per_cell: u64,
    /// Whether soft-drop points reach the attack conversion.
    pub soft_drop_counts_toward_attack: bool,
}

/// Fever values frozen for the match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeverRules {
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
    /// Every puzzle selectable this match.
    pub puzzles: FeverPuzzleBook,
}

/// Immutable data needed by a running match; it never observes asset reloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedMatchSpec {
    /// Selected profile identity.
    pub profile_id: RuleProfileId,
    /// Rule revision of the selected profile.
    pub rule_version: String,
    /// Seed every named random stream derives from.
    pub root_seed: u64,
    /// Board geometry shared by both players and both channels.
    pub board_geometry: BoardGeometry,
    /// Number of distinct ball colors.
    pub color_count: u8,
    /// Countdown before a round's first controllable tick.
    pub round_intro_ticks: u16,
    /// How long a round result stays before the next round.
    pub round_outro_ticks: u16,
    /// Falling-group and rotation timing.
    pub drop: DropTiming,
    /// Chain resolution phase timing.
    pub resolution: ResolutionRules,
    /// Score tables.
    pub scoring: ScoringRules,
    /// Score needed per nuisance ball before any decay.
    pub target_points: u64,
    /// Target-score decay.
    pub margin: MarginRules,
    /// Score-to-attack conversion.
    pub offense: OffenseRules,
    /// Nuisance drop geometry, batch limit and queue limit.
    pub nuisance: NuisanceRules,
    /// Fever values.
    pub fever: FeverRules,
    /// Selected character per participant slot.
    pub characters: [CharacterId; PARTICIPANT_SLOTS],
    /// Chain-power curves per participant slot.
    pub chain_power: [ChainPowerProfile; PARTICIPANT_SLOTS],
    /// Drop cycle per participant slot.
    pub drop_sets: [DropSet; PARTICIPANT_SLOTS],
    /// Digest tree entries this match used.
    pub digests: MatchDigests,
    /// Algorithm versions this match's state can be compared under.
    pub algorithms: AlgorithmVersions,
}

impl LockedMatchSpec {
    /// Freezes exactly the data selected by a request.
    ///
    /// Everything the running match needs is copied out here, so a later asset
    /// reload cannot reach a match that has already started.
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
        let book = library
            .puzzle_book(&request.rule_profile_id)
            .ok_or_else(|| {
                ConfigError::InvalidData(format!(
                    "rule profile {} has no Fever puzzle book",
                    request.rule_profile_id.0
                ))
            })?;

        let play_for = |character: &CharacterId| {
            library
                .character_play(&request.rule_profile_id, character)
                .ok_or_else(|| {
                    ConfigError::InvalidData(format!(
                        "missing gameplay data for character {}",
                        character.0
                    ))
                })
        };
        let plays = [
            play_for(&request.characters[0])?,
            play_for(&request.characters[1])?,
        ];
        let chain_power = [
            plays[0].chain_power_profile()?,
            plays[1].chain_power_profile()?,
        ];
        let drop_sets = [plays[0].drop_set.clone(), plays[1].drop_set.clone()];

        let board_geometry = profile.field.geometry().ok_or_else(|| {
            ConfigError::InvalidData("validated profile has invalid board geometry".into())
        })?;

        let digest_for = |character: &CharacterId| {
            library
                .play_digest(&request.rule_profile_id, character)
                .ok_or_else(|| {
                    ConfigError::InvalidData(format!(
                        "missing digest for character {}",
                        character.0
                    ))
                })
        };
        let digests = MatchDigests {
            root: library.root_digest(),
            roster: library.roster_digest(),
            profile: library
                .profile_digest(&request.rule_profile_id)
                .ok_or_else(|| ConfigError::InvalidData("missing profile digest".into()))?,
            puzzle_book: library
                .puzzle_book_digest(&request.rule_profile_id)
                .ok_or_else(|| ConfigError::InvalidData("missing puzzle book digest".into()))?,
            plays: [
                digest_for(&request.characters[0])?,
                digest_for(&request.characters[1])?,
            ],
        };

        Ok(Self {
            profile_id: request.rule_profile_id,
            rule_version: profile.rule_version.clone(),
            root_seed: request.root_seed,
            board_geometry,
            color_count: profile.field.color_count,
            round_intro_ticks: profile.round.intro_ticks,
            round_outro_ticks: profile.round.outro_ticks,
            drop: drop_timing(profile),
            resolution: resolution_rules(profile),
            scoring: ScoringRules::new(
                profile.scoring.color_bonus.clone(),
                profile.scoring.group_bonus.clone(),
            ),
            target_points: profile.scoring.target_points,
            margin: MarginRules {
                start_ticks: profile.scoring.margin.start_ticks,
                step_ticks: profile.scoring.margin.step_ticks,
                target_points_by_step: profile.scoring.margin.target_points_by_step.clone(),
            },
            offense: OffenseRules {
                soft_drop_points_per_cell: profile.offense.soft_drop_points_per_cell,
                soft_drop_counts_toward_attack: profile.offense.soft_drop_counts_toward_attack,
            },
            nuisance: NuisanceRules {
                drop_limit: profile.nuisance.drop_limit,
                queue_limit: profile.nuisance.queue_limit,
                columns: profile.field.width,
            },
            fever: FeverRules {
                gauge_capacity: profile.fever.gauge_capacity,
                initial_time_ticks: profile.fever.initial_time_ticks,
                min_time_ticks: profile.fever.min_time_ticks,
                max_time_ticks: profile.fever.max_time_ticks,
                min_level: profile.fever.min_level,
                max_level: profile.fever.max_level,
                offset_reward_ticks: profile.fever.offset_reward_ticks,
                all_clear_reward_ticks: profile.fever.all_clear_reward_ticks,
                all_clear_puzzle_id: profile.fever.all_clear_puzzle_id.clone(),
                level_ladder: profile.fever.level_ladder,
                puzzles: book.clone(),
            },
            characters: request.characters,
            chain_power,
            drop_sets,
            digests,
            algorithms: AlgorithmVersions::current(),
        })
    }
}

fn drop_timing(profile: &RuleProfile) -> DropTiming {
    DropTiming {
        next_queue_len: profile.drop.next_queue_len,
        natural_fall_ticks: profile.drop.natural_fall_ticks,
        soft_drop_ticks: profile.drop.soft_drop_ticks,
        horizontal_repeat_delay_ticks: profile.drop.horizontal_repeat_delay_ticks,
        horizontal_repeat_interval_ticks: profile.drop.horizontal_repeat_interval_ticks,
        horizontal_cooldown_ticks: profile.drop.horizontal_cooldown_ticks,
        lock_delay_ticks: profile.drop.lock_delay_ticks,
        lift_limit: profile.drop.lift_limit,
        split_delay_pivot_ticks: profile.drop.split_delay_pivot_ticks,
        split_delay_follower_ticks: profile.drop.split_delay_follower_ticks,
        rotation_cooldown_ticks: profile.rotation.cooldown_ticks,
        double_rotation_period: profile.rotation.double_rotation_period,
    }
}

fn resolution_rules(profile: &RuleProfile) -> ResolutionRules {
    ResolutionRules {
        clear_preview_ticks: profile.resolve.clear_preview_ticks,
        gravity_ticks_by_distance: profile.resolve.gravity_ticks_by_distance.clone(),
        clear_threshold: profile.resolve.clear_threshold,
    }
}
