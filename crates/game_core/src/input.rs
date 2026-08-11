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

    const fn mask(self) -> u8 {
        1 << self as u8
    }
}

/// Fixed-width set of logical actions for one participant and one tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerActions(u8);

impl PlayerActions {
    pub const EMPTY: Self = Self(0);

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickInputs {
    pub players: [PlayerActions; MAX_PLAYERS],
    pub len: u8,
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
