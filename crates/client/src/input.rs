//! Client-side physical input sampling and UI action types.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use game_core::input::{GameAction, PlayerActions};
use serde::{Deserialize, Serialize};

use crate::app_state::{
    AppState, AppTransitionCause, AppTransitionRequest, AppTransitionRequests, AppTransitionSet,
};
use crate::settings::{PlayerInputBindings, UserSettings};

#[derive(Debug, Default)]
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocalInputSampler>()
            .init_resource::<UiInputState>()
            .init_resource::<GamepadSlots>()
            .add_message::<UIActionEvent>()
            // Sampling must observe this frame's devices *before* this frame's
            // fixed ticks. Bevy's main schedule runs `RunFixedMainLoop` ahead of
            // `Update`, so capturing in `Update` would hand every rule tick the
            // previous frame's device state.
            .add_systems(
                RunFixedMainLoop,
                (install_settings_bindings, bind_gamepads, capture_devices)
                    .chain()
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            // The pause request is a state transition, so it stays with the
            // other requesters in `Update`; the edge it reads was recorded by
            // `capture_devices` earlier in the same frame.
            .add_systems(
                Update,
                submit_pause_request.in_set(AppTransitionSet::Request),
            )
            // Menu contexts only: in `Match` the same physical directions are
            // rules input, and must not also move UI focus.
            .add_systems(
                Update,
                emit_ui_actions.run_if(not(in_state(AppState::Match))),
            );
    }
}

/// Left-stick deflection past which a direction counts as held.
pub const STICK_THRESHOLD: f32 = 0.5;

/// Local player slots that can own a gamepad.
pub const LOCAL_PLAYERS: usize = 2;

/// Which local player each connected gamepad drives.
///
/// The binding is established once, when the pad appears, and holds for as
/// long as it stays connected. Deriving it from query order instead would let
/// an unrelated device change reassign a player mid-match.
#[derive(Debug, Default, Resource)]
pub struct GamepadSlots {
    by_pad: HashMap<Entity, usize>,
}

impl GamepadSlots {
    /// The local player this pad drives, if it is bound.
    #[must_use]
    pub fn slot(&self, pad: Entity) -> Option<usize> {
        self.by_pad.get(&pad).copied()
    }

    /// The pad bound to a local player, if any.
    #[must_use]
    pub fn pad(&self, player: usize) -> Option<Entity> {
        self.by_pad
            .iter()
            .find_map(|(pad, slot)| (*slot == player).then_some(*pad))
    }

    fn is_taken(&self, player: usize) -> bool {
        self.by_pad.values().any(|slot| *slot == player)
    }
}

/// Keep gamepad-to-player bindings current, clearing what a lost pad held.
///
/// A disconnected pad can never report a release, so anything it held would
/// otherwise stay pressed forever and keep producing actions.
pub fn bind_gamepads(
    gamepads: Query<Entity, With<Gamepad>>,
    mut slots: ResMut<GamepadSlots>,
    mut sampler: ResMut<LocalInputSampler>,
    mut ui: ResMut<UiInputState>,
) {
    let live: HashSet<Entity> = gamepads.iter().collect();

    let dropped: Vec<(Entity, usize)> = slots
        .by_pad
        .iter()
        .filter(|(pad, _)| !live.contains(pad))
        .map(|(pad, slot)| (*pad, *slot))
        .collect();
    for (pad, player) in dropped {
        slots.by_pad.remove(&pad);
        sampler.clear_gamepad_state(player);
        ui.clear_gamepad_state(player);
    }

    // Sorted so that two pads appearing in the same frame get slots in a
    // reproducible order rather than in whatever order the query yields.
    // Sorted by index rather than by `Entity`, whose ordering runs off an
    // opaque bit pattern that does not follow spawn order.
    let mut arriving: Vec<Entity> = live
        .into_iter()
        .filter(|pad| slots.slot(*pad).is_none())
        .collect();
    arriving.sort_by_key(|pad| pad.index_u32());
    for pad in arriving {
        let Some(player) = (0..LOCAL_PLAYERS).find(|slot| !slots.is_taken(*slot)) else {
            break;
        };
        slots.by_pad.insert(pad, player);
    }
}

/// The fixed inputs that propose a pause, for any local player.
///
/// `Pause` is not a `UIAction` and not a `GameAction`: `client::input` proposes
/// the state transition directly. `Escape` also means `Back` outside `Match`,
/// which is why the pause request is gated on the current `AppState` rather
/// than on the key alone.
#[must_use]
pub fn fixed_pause_inputs() -> [PhysicalInput; 2] {
    [
        PhysicalInput::gamepad("Start"),
        PhysicalInput::keyboard("Escape"),
    ]
}

/// Resolve a persisted input name to a live Bevy key.
///
/// Bindings store names rather than Bevy types because the workspace builds
/// Bevy without its `serialize` feature, so `KeyCode` has no `serde` impl to
/// persist. Names follow Bevy's own variant spelling. An unknown name resolves
/// to `None` and simply produces no action, which is the documented handling
/// for an unbound physical input.
#[must_use]
pub fn keyboard_from_name(name: &str) -> Option<KeyCode> {
    macro_rules! table {
        ($($variant:ident),* $(,)?) => {
            match name { $(stringify!($variant) => Some(KeyCode::$variant),)* _ => None }
        };
    }
    table!(
        KeyA,
        KeyB,
        KeyC,
        KeyD,
        KeyE,
        KeyF,
        KeyG,
        KeyH,
        KeyI,
        KeyJ,
        KeyK,
        KeyL,
        KeyM,
        KeyN,
        KeyO,
        KeyP,
        KeyQ,
        KeyR,
        KeyS,
        KeyT,
        KeyU,
        KeyV,
        KeyW,
        KeyX,
        KeyY,
        KeyZ,
        Digit0,
        Digit1,
        Digit2,
        Digit3,
        Digit4,
        Digit5,
        Digit6,
        Digit7,
        Digit8,
        Digit9,
        ArrowUp,
        ArrowDown,
        ArrowLeft,
        ArrowRight,
        Numpad0,
        Numpad1,
        Numpad2,
        Numpad3,
        Numpad4,
        Numpad5,
        Numpad6,
        Numpad7,
        Numpad8,
        Numpad9,
        NumpadEnter,
        Space,
        Enter,
        Escape,
        Tab,
        Backspace,
        ShiftLeft,
        ShiftRight,
        ControlLeft,
        ControlRight,
        AltLeft,
        AltRight,
        Comma,
        Period,
        Slash,
        Semicolon,
        Quote,
        BracketLeft,
        BracketRight,
        Minus,
        Equal,
        Backquote,
    )
}

/// Resolve a persisted input name to a live Bevy gamepad button.
#[must_use]
pub fn gamepad_button_from_name(name: &str) -> Option<GamepadButton> {
    macro_rules! table {
        ($($variant:ident),* $(,)?) => {
            match name { $(stringify!($variant) => Some(GamepadButton::$variant),)* _ => None }
        };
    }
    table!(
        South,
        East,
        North,
        West,
        LeftTrigger,
        LeftTrigger2,
        RightTrigger,
        RightTrigger2,
        Select,
        Start,
        Mode,
        LeftThumb,
        RightThumb,
        DPadUp,
        DPadDown,
        DPadLeft,
        DPadRight,
    )
}

/// Fixed keyboard direction keys for a local player slot.
///
/// These are not user-configurable, and they feed both domains: in gameplay
/// they become `GameAction::Left`/`Right`, in menus `UIAction` focus moves.
#[must_use]
pub fn fixed_keyboard_directions(player: usize) -> Option<[(KeyCode, FixedDirection); 4]> {
    match player {
        0 => Some([
            (KeyCode::KeyA, FixedDirection::Left),
            (KeyCode::KeyD, FixedDirection::Right),
            (KeyCode::KeyW, FixedDirection::Up),
            (KeyCode::KeyS, FixedDirection::Down),
        ]),
        1 => Some([
            (KeyCode::ArrowLeft, FixedDirection::Left),
            (KeyCode::ArrowRight, FixedDirection::Right),
            (KeyCode::ArrowUp, FixedDirection::Up),
            (KeyCode::ArrowDown, FixedDirection::Down),
        ]),
        _ => None,
    }
}

/// Fixed gamepad D-pad direction buttons.
const FIXED_DPAD_DIRECTIONS: [(GamepadButton, FixedDirection); 4] = [
    (GamepadButton::DPadLeft, FixedDirection::Left),
    (GamepadButton::DPadRight, FixedDirection::Right),
    (GamepadButton::DPadUp, FixedDirection::Up),
    (GamepadButton::DPadDown, FixedDirection::Down),
];

/// Fixed keyboard confirm/back keys for a local player slot.
#[must_use]
pub fn fixed_keyboard_menu_keys(player: usize) -> Option<[(KeyCode, UIAction); 2]> {
    match player {
        0 => Some([
            (KeyCode::Space, UIAction::Confirm),
            (KeyCode::ShiftLeft, UIAction::Back),
        ]),
        1 => Some([
            (KeyCode::Enter, UIAction::Confirm),
            (KeyCode::ShiftRight, UIAction::Back),
        ]),
        _ => None,
    }
}

/// Fixed gamepad confirm/back buttons, shared by every local player.
const FIXED_GAMEPAD_MENU_BUTTONS: [(GamepadButton, UIAction); 2] = [
    (GamepadButton::South, UIAction::Confirm),
    (GamepadButton::East, UIAction::Back),
];

/// A UI action produced by a local player in a menu context.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UIActionEvent {
    pub player: usize,
    pub action: UIAction,
}

/// Held UI sources, so focus moves once per press instead of once per frame.
#[derive(Debug, Default, Resource)]
pub struct UiInputState {
    held: HashSet<(usize, PhysicalInput, UIAction)>,
}

impl UiInputState {
    /// Report a source's state, returning `true` on the rising edge only.
    fn edge(&mut self, player: usize, source: PhysicalInput, action: UIAction, held: bool) -> bool {
        let key = (player, source, action);
        if held {
            self.held.insert(key)
        } else {
            self.held.remove(&key);
            false
        }
    }

    /// Forget what a player's gamepad held, so a later pad starts from rest.
    ///
    /// A stale entry here would swallow the first press after a reconnect: the
    /// rising edge is only reported when the source was not already held.
    fn clear_gamepad_state(&mut self, player: usize) {
        self.held.retain(|(slot, source, _)| {
            *slot != player || !matches!(source, PhysicalInput::Gamepad(_))
        });
    }
}

/// Give the sampler the player's bindings once settings are available.
///
/// Without this the sampler starts with no bindings and no key would ever
/// produce an action. It only fills an empty sampler: tests install their own
/// bindings, and pushing a settings change onto a running sampler belongs to
/// the settings system rather than here.
pub fn install_settings_bindings(
    settings: Res<UserSettings>,
    mut sampler: ResMut<LocalInputSampler>,
) {
    if sampler.bindings.is_empty() {
        sampler.set_bindings(settings.players.to_vec());
    }
}

/// Resolve each local player's gamepad through its stable slot binding.
fn pads_by_player<'a>(
    gamepads: &'a Query<(Entity, &Gamepad)>,
    slots: &GamepadSlots,
) -> [Option<&'a Gamepad>; LOCAL_PLAYERS] {
    let mut pads = [None; LOCAL_PLAYERS];
    for (entity, pad) in gamepads.iter() {
        if let Some(cell) = slots.slot(entity).and_then(|slot| pads.get_mut(slot)) {
            *cell = Some(pad);
        }
    }
    pads
}

/// Read real keyboard and gamepad state into the sampler once per frame.
///
/// The sampler owns the fixed-tick semantics; this system only reports what is
/// physically held right now and which press edges happened this frame.
/// Gamepads reach their player through [`GamepadSlots`], not through query
/// order, so a device change elsewhere cannot move a player's input.
pub fn capture_devices(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(Entity, &Gamepad)>,
    slots: Res<GamepadSlots>,
    mut sampler: ResMut<LocalInputSampler>,
) {
    let pads = pads_by_player(&gamepads, &slots);

    // Collected first: resolving a binding borrows the sampler that the
    // press/release calls below need mutably.
    let mut configurable: Vec<(usize, PhysicalInput, bool, bool)> = Vec::new();
    for (player, bindings) in sampler.bindings.iter().enumerate() {
        let pad = pads.get(player).copied().flatten();
        for input in bindings.bindings.values().flatten() {
            let (held, edge) = match input {
                PhysicalInput::Keyboard(name) => keyboard_from_name(name)
                    .map_or((false, false), |code| {
                        (keyboard.pressed(code), keyboard.just_pressed(code))
                    }),
                PhysicalInput::Gamepad(name) => gamepad_button_from_name(name)
                    .zip(pad)
                    .map_or((false, false), |(button, pad)| {
                        (pad.pressed(button), pad.just_pressed(button))
                    }),
            };
            configurable.push((player, input.clone(), held, edge));
        }
    }
    for (player, input, held, edge) in configurable {
        // A press that also ended this frame still owes the rules layer one
        // action, so the edge is reported even though nothing is held now.
        // `press` only records the edge on the transition, so the extra call
        // when the input is still held is a no-op.
        if edge {
            sampler.press(player, input.clone());
        }
        if held {
            sampler.press(player, input);
        } else {
            sampler.release(player, &input);
        }
    }

    let players = sampler.bindings.len().max(LOCAL_PLAYERS);
    for player in 0..players {
        let pad = pads.get(player).copied().flatten();

        for (code, direction) in fixed_keyboard_directions(player).into_iter().flatten() {
            let source = PhysicalInput::keyboard(format!("{code:?}"));
            if keyboard.pressed(code) {
                sampler.press_fixed_direction(player, source, direction);
            } else {
                sampler.release_fixed_direction(player, &source, direction);
            }
        }

        let Some(pad) = pad else { continue };

        for (button, direction) in FIXED_DPAD_DIRECTIONS {
            let source = PhysicalInput::gamepad(format!("{button:?}"));
            if pad.pressed(button) {
                sampler.press_fixed_direction(player, source, direction);
            } else {
                sampler.release_fixed_direction(player, &source, direction);
            }
        }

        // One stick reports two axes, so each axis is its own source: holding
        // the stick left must not also register as a vertical direction.
        let stick = pad.left_stick();
        for (value, negative, positive, axis) in [
            (stick.x, FixedDirection::Left, FixedDirection::Right, "X"),
            (stick.y, FixedDirection::Down, FixedDirection::Up, "Y"),
        ] {
            let source = PhysicalInput::gamepad(format!("LeftStick{axis}"));
            for (direction, active) in [
                (negative, value < -STICK_THRESHOLD),
                (positive, value > STICK_THRESHOLD),
            ] {
                if active {
                    sampler.press_fixed_direction(player, source.clone(), direction);
                } else {
                    sampler.release_fixed_direction(player, &source, direction);
                }
            }
        }
    }

    // The sampler derives the edge, so holding the key proposes one pause.
    for input in fixed_pause_inputs() {
        let held = match &input {
            PhysicalInput::Keyboard(name) => {
                keyboard_from_name(name).is_some_and(|code| keyboard.pressed(code))
            }
            PhysicalInput::Gamepad(name) => gamepad_button_from_name(name)
                .is_some_and(|button| pads.iter().flatten().any(|pad| pad.pressed(button))),
        };
        sampler.update_pause_input(&input, held);
    }
}

/// Turn fixed physical inputs into `UIAction`s while outside `Match`.
///
/// The same physical direction that drives `GameAction::Left`/`Right` in
/// gameplay drives focus movement here; the input context decides which domain
/// consumes it, and the two never merge. Emission is edge-triggered so holding
/// a direction moves focus once rather than every frame.
pub fn emit_ui_actions(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<(Entity, &Gamepad)>,
    slots: Res<GamepadSlots>,
    mut state: ResMut<UiInputState>,
    mut writer: MessageWriter<UIActionEvent>,
) {
    let pads = pads_by_player(&gamepads, &slots);

    for player in 0..LOCAL_PLAYERS {
        let pad = pads.get(player).copied().flatten();
        let mut emit = |source: PhysicalInput, action: UIAction, held: bool| {
            if state.edge(player, source, action, held) {
                writer.write(UIActionEvent { player, action });
            }
        };

        for (code, direction) in fixed_keyboard_directions(player).into_iter().flatten() {
            if let Some(ContextAction::Ui(action)) =
                interpret_direction(direction, InputContext::Menu)
            {
                let source = PhysicalInput::keyboard(format!("{code:?}"));
                emit(source, action, keyboard.pressed(code));
            }
        }
        for (code, action) in fixed_keyboard_menu_keys(player).into_iter().flatten() {
            let source = PhysicalInput::keyboard(format!("{code:?}"));
            emit(source, action, keyboard.pressed(code));
        }

        let Some(pad) = pad else { continue };

        for (button, direction) in FIXED_DPAD_DIRECTIONS {
            if let Some(ContextAction::Ui(action)) =
                interpret_direction(direction, InputContext::Menu)
            {
                let source = PhysicalInput::gamepad(format!("{button:?}"));
                emit(source, action, pad.pressed(button));
            }
        }
        for (button, action) in FIXED_GAMEPAD_MENU_BUTTONS {
            let source = PhysicalInput::gamepad(format!("{button:?}"));
            emit(source, action, pad.pressed(button));
        }

        let stick = pad.left_stick();
        for (value, negative, positive, axis) in [
            (stick.x, FixedDirection::Left, FixedDirection::Right, "X"),
            (stick.y, FixedDirection::Down, FixedDirection::Up, "Y"),
        ] {
            let source = PhysicalInput::gamepad(format!("LeftStick{axis}"));
            for (direction, active) in [
                (negative, value < -STICK_THRESHOLD),
                (positive, value > STICK_THRESHOLD),
            ] {
                if let Some(ContextAction::Ui(action)) =
                    interpret_direction(direction, InputContext::Menu)
                {
                    emit(source.clone(), action, active);
                }
            }
        }
    }
}

/// Forward a pending pause press edge straight to the state machine.
pub fn submit_pause_request(
    state: Res<State<AppState>>,
    mut sampler: ResMut<LocalInputSampler>,
    mut requests: ResMut<AppTransitionRequests>,
) {
    if let Some(request) = sampler.take_pause_request(*state.get()) {
        requests.submit(request.target, request.cause);
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
    /// Fixed-binding directions are keyed by their physical source so that two
    /// sources meaning the same direction merge into one logical action.
    fixed_directions: HashSet<(usize, PhysicalInput, FixedDirection)>,
    pending_edges: Vec<PlayerActions>,
    pause_pending: bool,
    /// Fixed pause inputs currently held, so a hold proposes exactly one pause.
    pause_held: HashSet<PhysicalInput>,
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

    /// Replace the binding table, resizing the per-player edge buffers with it.
    pub fn set_bindings(&mut self, bindings: Vec<PlayerInputBindings>) {
        self.pending_edges
            .resize(bindings.len(), PlayerActions::EMPTY);
        self.bindings = bindings;
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

    pub fn press_fixed_direction(
        &mut self,
        player: usize,
        source: PhysicalInput,
        direction: FixedDirection,
    ) {
        self.fixed_directions.insert((player, source, direction));
    }

    pub fn release_fixed_direction(
        &mut self,
        player: usize,
        source: &PhysicalInput,
        direction: FixedDirection,
    ) {
        self.fixed_directions
            .remove(&(player, source.clone(), direction));
    }

    /// Drop everything a player's gamepad was holding.
    ///
    /// Pending press edges survive: those are completed presses the player
    /// actually made, still owed to the rules layer. Only held state, which
    /// only means something while the device is there to report it, is lost.
    pub fn clear_gamepad_state(&mut self, player: usize) {
        self.pressed
            .retain(|(slot, input)| *slot != player || !matches!(input, PhysicalInput::Gamepad(_)));
        self.fixed_directions.retain(|(slot, source, _)| {
            *slot != player || !matches!(source, PhysicalInput::Gamepad(_))
        });
    }

    /// Record a press edge of a fixed pause input; other inputs are ignored.
    pub fn press_pause(&mut self, source: &PhysicalInput) {
        if fixed_pause_inputs().contains(source) {
            self.pause_pending = true;
        }
    }

    /// Track a fixed pause input's held state, proposing a pause on the edge.
    ///
    /// The edge is derived here rather than read from Bevy's `just_pressed`,
    /// because that flag is cleared in `PreUpdate` and would depend on the
    /// device event landing in the same frame the capture system runs. Holding
    /// the key still proposes exactly one pause.
    pub fn update_pause_input(&mut self, source: &PhysicalInput, held: bool) {
        if !fixed_pause_inputs().contains(source) {
            return;
        }
        if held {
            if self.pause_held.insert(source.clone()) {
                self.pause_pending = true;
            }
        } else {
            self.pause_held.remove(source);
        }
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

        // Several physical sources may report the same direction; inserting into
        // the shared bit set merges them into a single logical action.
        for (player, _source, direction) in &self.fixed_directions {
            let Some(player_actions) = sampled.get_mut(*player) else {
                continue;
            };
            if let Some(ContextAction::Game(action)) =
                interpret_direction(*direction, InputContext::Gameplay)
            {
                player_actions.insert(action);
            }
        }

        for (player, player_actions) in sampled.iter_mut().enumerate() {
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
