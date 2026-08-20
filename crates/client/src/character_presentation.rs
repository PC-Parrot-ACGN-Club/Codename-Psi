//! Versioned, client-only character presentation data and fallback lookup.

use std::collections::{BTreeMap, BTreeSet};

use game_core::config::{CharacterId, CharacterIdentity, Roster};
use serde::Deserialize;

use crate::data::{DataErrorCause, DataResolution};

pub const CHARACTER_PRESENTATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum PoseKind {
    Idle,
    Spell,
    Offset,
    Damage,
    Advantage,
}

pub const POSES: [PoseKind; 5] = [
    PoseKind::Idle,
    PoseKind::Spell,
    PoseKind::Offset,
    PoseKind::Damage,
    PoseKind::Advantage,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum AudioCue {
    RoundStart,
    Clear,
    Chain,
    FeverEnter,
    Win,
    Lose,
}

pub const AUDIO_CUES: [AudioCue; 6] = [
    AudioCue::RoundStart,
    AudioCue::Clear,
    AudioCue::Chain,
    AudioCue::FeverEnter,
    AudioCue::Win,
    AudioCue::Lose,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum BadgeShape {
    Hexagon,
    Triangle,
    Circle,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BadgeSpec {
    pub shape: BadgeShape,
    pub stroke_width: u8,
    pub glyph: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct PoseSpec {
    pub offset: i16,
    pub scale: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterPresentation {
    pub id: CharacterId,
    pub primary_color: RgbColor,
    pub secondary_color: RgbColor,
    pub badge: BadgeSpec,
    pub poses: BTreeMap<PoseKind, PoseSpec>,
    pub audio: BTreeMap<AudioCue, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CharacterPresentationCatalog(BTreeMap<CharacterId, CharacterPresentation>);

impl CharacterPresentationCatalog {
    #[must_use]
    pub fn from_entries(entries: impl IntoIterator<Item = CharacterPresentation>) -> Self {
        Self(
            entries
                .into_iter()
                .map(|entry| (entry.id.clone(), entry))
                .collect(),
        )
    }

    #[must_use]
    pub fn get(&self, id: &CharacterId) -> Option<&CharacterPresentation> {
        self.0.get(id)
    }
}

#[derive(Debug, Deserialize)]
struct RawCatalog {
    schema_version: u32,
    characters: Vec<RawCharacter>,
}

#[derive(Debug, Deserialize)]
struct RawCharacter {
    id: CharacterId,
    primary_color: RgbColor,
    secondary_color: RgbColor,
    badge: BadgeSpec,
    poses: RawPoses,
    audio: RawAudio,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawPoses {
    idle: PoseSpec,
    spell: PoseSpec,
    offset: PoseSpec,
    damage: PoseSpec,
    advantage: PoseSpec,
}

impl RawPoses {
    fn finish(self, id: &CharacterId) -> Result<BTreeMap<PoseKind, PoseSpec>, DataErrorCause> {
        let fields = [
            (PoseKind::Idle, "idle", self.idle),
            (PoseKind::Spell, "spell", self.spell),
            (PoseKind::Offset, "offset", self.offset),
            (PoseKind::Damage, "damage", self.damage),
            (PoseKind::Advantage, "advantage", self.advantage),
        ];
        fields
            .into_iter()
            .map(|(kind, name, value)| {
                (value.scale != 0).then_some((kind, value)).ok_or_else(|| {
                    DataErrorCause::InvalidData(format!(
                        "character {} is missing required pose {name}",
                        id.0
                    ))
                })
            })
            .collect()
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawAudio {
    round_start: String,
    clear: String,
    chain: String,
    fever_enter: String,
    win: String,
    lose: String,
}

impl RawAudio {
    fn finish(self, id: &CharacterId) -> Result<BTreeMap<AudioCue, String>, DataErrorCause> {
        let fields = [
            (AudioCue::RoundStart, "round_start", self.round_start),
            (AudioCue::Clear, "clear", self.clear),
            (AudioCue::Chain, "chain", self.chain),
            (AudioCue::FeverEnter, "fever_enter", self.fever_enter),
            (AudioCue::Win, "win", self.win),
            (AudioCue::Lose, "lose", self.lose),
        ];
        fields
            .into_iter()
            .map(|(kind, name, value)| {
                (!value.is_empty()).then_some((kind, value)).ok_or_else(|| {
                    DataErrorCause::InvalidData(format!(
                        "character {} is missing required audio cue {name}",
                        id.0
                    ))
                })
            })
            .collect()
    }
}

pub fn parse_character_presentations(
    source: &str,
    roster: &Roster,
) -> Result<CharacterPresentationCatalog, DataErrorCause> {
    let raw: RawCatalog =
        ron::from_str(source).map_err(|error| DataErrorCause::Parse(error.to_string()))?;
    if raw.schema_version != CHARACTER_PRESENTATION_SCHEMA_VERSION {
        return Err(DataErrorCause::UnsupportedSchema {
            found: raw.schema_version,
            supported: CHARACTER_PRESENTATION_SCHEMA_VERSION,
        });
    }
    let rostered: BTreeSet<_> = roster
        .characters
        .iter()
        .map(|entry| entry.id.clone())
        .collect();
    let mut entries = BTreeMap::new();
    for raw in raw.characters {
        if !rostered.contains(&raw.id) {
            return Err(DataErrorCause::InvalidData(format!(
                "character {} is not present in the roster",
                raw.id.0
            )));
        }
        let poses = raw.poses.finish(&raw.id)?;
        let audio = raw.audio.finish(&raw.id)?;
        let entry = CharacterPresentation {
            id: raw.id.clone(),
            primary_color: raw.primary_color,
            secondary_color: raw.secondary_color,
            badge: raw.badge,
            poses,
            audio,
        };
        if entries.insert(raw.id.clone(), entry).is_some() {
            return Err(DataErrorCause::InvalidData(format!(
                "character {} appears more than once",
                raw.id.0
            )));
        }
    }
    Ok(CharacterPresentationCatalog(entries))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCharacterPresentation {
    pub data: CharacterPresentation,
    pub fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterPresentationDiagnostic {
    pub character_id: CharacterId,
    pub reason: String,
}

pub struct CharacterPresentationResolver {
    resolution: DataResolution<CharacterPresentationCatalog>,
    diagnosed: BTreeSet<CharacterId>,
    diagnostics: Vec<CharacterPresentationDiagnostic>,
}

impl CharacterPresentationResolver {
    #[must_use]
    pub fn new(resolution: DataResolution<CharacterPresentationCatalog>) -> Self {
        Self {
            resolution,
            diagnosed: BTreeSet::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn resolve(
        &mut self,
        identity: &CharacterIdentity,
        slot: usize,
    ) -> ResolvedCharacterPresentation {
        if let Some(data) = self
            .resolution
            .loaded()
            .and_then(|catalog| catalog.get(&identity.id))
        {
            return ResolvedCharacterPresentation {
                data: data.clone(),
                fallback: false,
            };
        }

        if self.diagnosed.insert(identity.id.clone()) {
            let reason = self.resolution.error().map_or_else(
                || "character is missing from the presentation catalog".to_owned(),
                |error| format!("presentation catalog failed: {:?}", error.cause),
            );
            self.diagnostics.push(CharacterPresentationDiagnostic {
                character_id: identity.id.clone(),
                reason,
            });
        }
        ResolvedCharacterPresentation {
            data: fallback(identity, slot),
            fallback: true,
        }
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[CharacterPresentationDiagnostic] {
        &self.diagnostics
    }
}

fn fallback(identity: &CharacterIdentity, slot: usize) -> CharacterPresentation {
    let primary_color = if slot == 0 {
        RgbColor {
            r: 64,
            g: 160,
            b: 255,
        }
    } else {
        RgbColor {
            r: 255,
            g: 144,
            b: 64,
        }
    };
    let neutral = PoseSpec {
        offset: 0,
        scale: 100,
    };
    CharacterPresentation {
        id: identity.id.clone(),
        primary_color,
        secondary_color: RgbColor {
            r: 32,
            g: 32,
            b: 32,
        },
        badge: BadgeSpec {
            shape: BadgeShape::Circle,
            stroke_width: 4,
            glyph: identity
                .display_name_key
                .chars()
                .next()
                .unwrap_or('?')
                .to_string(),
        },
        poses: POSES.into_iter().map(|kind| (kind, neutral)).collect(),
        audio: AUDIO_CUES
            .into_iter()
            .map(|cue| (cue, "silent".to_owned()))
            .collect(),
    }
}
