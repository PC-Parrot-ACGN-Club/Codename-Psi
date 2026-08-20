//! Device-independent input consumed by the deterministic rules layer.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum number of stable participant slots in one match.
pub const MAX_PLAYERS: usize = 8;

/// A logical action understood by the rules layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GameAction {
    Left,
    Right,
    SoftDrop,
    HardDrop,
    RotateClockwise,
    RotateCounterClockwise,
}

impl GameAction {
    /// Actions exposed by the user-configurable binding UI.
    pub const CONFIGURABLE: [Self; 4] = [
        Self::SoftDrop,
        Self::HardDrop,
        Self::RotateClockwise,
        Self::RotateCounterClockwise,
    ];

    /// Whether this action belongs to the configurable binding surface.
    #[must_use]
    pub const fn is_configurable(self) -> bool {
        matches!(
            self,
            Self::SoftDrop | Self::HardDrop | Self::RotateClockwise | Self::RotateCounterClockwise
        )
    }

    /// The bit this action occupies in the stable encoding.
    ///
    /// Written out rather than derived from the variant's position, because
    /// the encoding outlives this declaration order: reordering the enum for
    /// readability must not silently rewrite logs, checksums or wire payloads.
    const fn mask(self) -> u8 {
        match self {
            Self::Left => 1 << 0,
            Self::Right => 1 << 1,
            Self::SoftDrop => 1 << 2,
            Self::HardDrop => 1 << 3,
            Self::RotateClockwise => 1 << 4,
            Self::RotateCounterClockwise => 1 << 5,
        }
    }
}

/// Fixed-width set of logical actions for one participant and one tick.
///
/// `Deserialize` is written by hand so every public way in goes through
/// [`PlayerActions::from_bits`]. A derived one would accept the reserved bits
/// that `from_bits` exists to reject.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct PlayerActions(u8);

impl<'de> Deserialize<'de> for PlayerActions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = u8::deserialize(deserializer)?;
        Self::from_bits(bits).ok_or_else(|| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Unsigned(u64::from(bits)),
                &"a bit set with the reserved bits 6-7 clear",
            )
        })
    }
}

impl PlayerActions {
    pub const EMPTY: Self = Self(0);

    /// Bits 6-7 are reserved by the stable encoding and always read as zero.
    const RESERVED_MASK: u8 = 0b1100_0000;

    /// The stable wire/log encoding: bit 0-5 in `GameAction` order.
    ///
    /// Deterministic verification logs, snapshot checksums and later network
    /// input encoding all depend on this value, so it is part of the crate's
    /// public contract rather than an implementation detail.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Rebuild a set from the stable encoding.
    ///
    /// # Errors
    ///
    /// Returns `None` when a reserved bit is set, so a corrupt or
    /// future-versioned payload cannot silently decode into a valid set.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        if bits & Self::RESERVED_MASK != 0 {
            None
        } else {
            Some(Self(bits))
        }
    }

    #[must_use]
    pub const fn from_action(action: GameAction) -> Self {
        Self(action.mask())
    }

    #[must_use]
    pub fn from_actions(actions: impl IntoIterator<Item = GameAction>) -> Self {
        actions.into_iter().collect()
    }

    #[must_use]
    pub const fn contains(self, action: GameAction) -> bool {
        self.0 & action.mask() != 0
    }

    pub fn insert(&mut self, action: GameAction) {
        self.0 |= action.mask();
    }

    pub fn remove(&mut self, action: GameAction) {
        self.0 &= !action.mask();
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Apply the source-independent conflict rules from the input spec.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        if self.contains(GameAction::Left) && self.contains(GameAction::Right) {
            self.remove(GameAction::Left);
            self.remove(GameAction::Right);
        }
        if self.contains(GameAction::RotateClockwise)
            && self.contains(GameAction::RotateCounterClockwise)
        {
            self.remove(GameAction::RotateClockwise);
            self.remove(GameAction::RotateCounterClockwise);
        }
        if self.contains(GameAction::SoftDrop) && self.contains(GameAction::HardDrop) {
            self.remove(GameAction::SoftDrop);
        }
        self
    }
}

impl From<GameAction> for PlayerActions {
    fn from(value: GameAction) -> Self {
        Self::from_action(value)
    }
}

impl FromIterator<GameAction> for PlayerActions {
    fn from_iter<T: IntoIterator<Item = GameAction>>(iter: T) -> Self {
        let mut result = Self::EMPTY;
        for action in iter {
            result.insert(action);
        }
        result
    }
}

impl std::ops::BitOr for PlayerActions {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// Failure to construct a fixed-capacity tick input value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TickInputsError {
    #[error("participant count {found} exceeds maximum {maximum}")]
    TooManyPlayers { found: usize, maximum: usize },
}

/// Inputs for every stable participant slot in one rules tick.
///
/// The fields stay private so `players[len..]` cannot be observed or
/// corrupted from outside: keeping the tail empty is what gives identical
/// logical inputs one canonical byte form, which equality, hashing,
/// checksums and later network encoding all rely on. Slot occupancy is a
/// question for [`TickInputs::player`], which separates "no such
/// participant" from "that participant did nothing".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickInputs {
    players: [PlayerActions; MAX_PLAYERS],
    len: u8,
}

impl TickInputs {
    pub const EMPTY: Self = Self {
        players: [PlayerActions::EMPTY; MAX_PLAYERS],
        len: 0,
    };

    pub fn new(actions: impl AsRef<[PlayerActions]>) -> Result<Self, TickInputsError> {
        let actions = actions.as_ref();
        if actions.len() > MAX_PLAYERS {
            return Err(TickInputsError::TooManyPlayers {
                found: actions.len(),
                maximum: MAX_PLAYERS,
            });
        }
        let mut players = [PlayerActions::EMPTY; MAX_PLAYERS];
        players[..actions.len()].copy_from_slice(actions);
        Ok(Self {
            players,
            len: actions.len() as u8,
        })
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn active(&self) -> &[PlayerActions] {
        &self.players[..self.len()]
    }

    #[must_use]
    pub fn player(&self, slot: usize) -> Option<PlayerActions> {
        self.active().get(slot).copied()
    }
}

impl Default for TickInputs {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tail is unobservable from outside, so the canonicalization rule it
    /// exists for is guarded here, next to the code that has to maintain it.
    #[test]
    fn unused_slots_stay_empty() {
        let inputs = TickInputs::new([PlayerActions::from_action(GameAction::Left)])
            .expect("one participant fits");

        assert!(
            inputs.players[inputs.len()..]
                .iter()
                .all(|actions| *actions == PlayerActions::EMPTY),
            "slots past len must stay empty to keep one canonical encoding"
        );
    }

    #[test]
    fn identical_logical_inputs_compare_equal() {
        let actions = PlayerActions::from_action(GameAction::HardDrop);

        assert_eq!(
            TickInputs::new([actions]).expect("one participant fits"),
            TickInputs::new([actions]).expect("one participant fits"),
        );
    }
}
