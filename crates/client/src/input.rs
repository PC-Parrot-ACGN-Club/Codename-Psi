//! Client-side physical input sampling and UI action types.

use std::collections::HashSet;

use bevy::prelude::*;
use game_core::input::{GameAction, PlayerActions};
use serde::{Deserialize, Serialize};

use crate::app_state::{AppState, AppTransitionCause, AppTransitionRequest};
use crate::settings::PlayerInputBindings;

#[derive(Debug, Default)]
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocalInputSampler>();
    }
}

/// UI-domain actions, kept separate from rules input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UIAction {
    Left,
    Right,
    Up,
    Down,
    Confirm,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputContext {
    Gameplay,
    Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixedDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PhysicalInput {
    Keyboard(String),
    Gamepad(String),
}

impl PhysicalInput {
    #[must_use]
    pub fn keyboard(code: impl Into<String>) -> Self {
        Self::Keyboard(code.into())
    }

    #[must_use]
    pub fn gamepad(button: impl Into<String>) -> Self {
        Self::Gamepad(button.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextAction {
    Game(GameAction),
    Ui(UIAction),
}

#[must_use]
pub const fn interpret_direction(
    direction: FixedDirection,
    context: InputContext,
) -> Option<ContextAction> {
    match (context, direction) {
        (InputContext::Gameplay, FixedDirection::Left) => {
            Some(ContextAction::Game(GameAction::Left))
        }
        (InputContext::Gameplay, FixedDirection::Right) => {
            Some(ContextAction::Game(GameAction::Right))
        }
        (InputContext::Gameplay, FixedDirection::Up | FixedDirection::Down) => None,
        (InputContext::Menu, FixedDirection::Left) => Some(ContextAction::Ui(UIAction::Left)),
        (InputContext::Menu, FixedDirection::Right) => Some(ContextAction::Ui(UIAction::Right)),
        (InputContext::Menu, FixedDirection::Up) => Some(ContextAction::Ui(UIAction::Up)),
        (InputContext::Menu, FixedDirection::Down) => Some(ContextAction::Ui(UIAction::Down)),
    }
}

/// Mutable sampling state exposed for pure component tests.
#[derive(Debug, Default, Resource)]
pub struct LocalInputSampler {
    pub bindings: Vec<PlayerInputBindings>,
    pressed: HashSet<(usize, PhysicalInput)>,
    fixed_directions: HashSet<(usize, FixedDirection)>,
    pending_edges: Vec<PlayerActions>,
    pause_pending: bool,
}

impl LocalInputSampler {
    #[must_use]
    pub fn new(bindings: Vec<PlayerInputBindings>) -> Self {
        let pending_edges = vec![PlayerActions::EMPTY; bindings.len()];
        Self {
            bindings,
            pending_edges,
            ..Default::default()
        }
    }

    pub fn press(&mut self, player: usize, input: PhysicalInput) {
        let first_press = self.pressed.insert((player, input.clone()));
        if first_press {
            for action in self.bound_actions(player, &input) {
                if is_edge_action(action) {
                    self.ensure_player(player);
                    self.pending_edges[player].insert(action);
                }
            }
        }
    }

    pub fn release(&mut self, player: usize, input: &PhysicalInput) {
        self.pressed.remove(&(player, input.clone()));
    }

    pub fn press_fixed_direction(&mut self, player: usize, direction: FixedDirection) {
        self.fixed_directions.insert((player, direction));
    }

    pub fn release_fixed_direction(&mut self, player: usize, direction: FixedDirection) {
        self.fixed_directions.remove(&(player, direction));
    }

    pub fn press_pause(&mut self) {
        self.pause_pending = true;
    }

    #[must_use]
    pub fn take_pause_request(&mut self, state: AppState) -> Option<AppTransitionRequest> {
        let pending = std::mem::take(&mut self.pause_pending);
        (pending && state == AppState::Match).then_some(AppTransitionRequest {
            target: AppState::Paused,
            cause: AppTransitionCause::PauseRequested,
        })
    }

    /// Sample raw actions. Callers pass the result through `PlayerActions::normalized`.
    pub fn sample_fixed(&mut self) -> Vec<PlayerActions> {
        let count = self.bindings.len().max(self.pending_edges.len());
        let mut sampled = vec![PlayerActions::EMPTY; count];

        for (player, player_actions) in sampled.iter_mut().enumerate() {
            if self
                .fixed_directions
                .contains(&(player, FixedDirection::Left))
            {
                player_actions.insert(GameAction::Left);
            }
            if self
                .fixed_directions
                .contains(&(player, FixedDirection::Right))
            {
                player_actions.insert(GameAction::Right);
            }
            if let Some(edges) = self.pending_edges.get_mut(player) {
                *player_actions = *player_actions | *edges;
                *edges = PlayerActions::EMPTY;
            }
        }

        for (player, input) in &self.pressed {
            if *player >= sampled.len() {
                continue;
            }
            for action in self.bound_actions(*player, input) {
                if !is_edge_action(action) {
                    sampled[*player].insert(action);
                }
            }
        }
        sampled
    }

    fn bound_actions(&self, player: usize, input: &PhysicalInput) -> Vec<GameAction> {
        self.bindings
            .get(player)
            .map(|bindings| bindings.actions_for(input).collect())
            .unwrap_or_default()
    }

    fn ensure_player(&mut self, player: usize) {
        if self.pending_edges.len() <= player {
            self.pending_edges.resize(player + 1, PlayerActions::EMPTY);
        }
    }
}

const fn is_edge_action(action: GameAction) -> bool {
    matches!(
        action,
        GameAction::HardDrop | GameAction::RotateClockwise | GameAction::RotateCounterClockwise
    )
}
