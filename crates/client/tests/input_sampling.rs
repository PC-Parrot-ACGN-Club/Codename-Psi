//! Local sampling of fixed-binding directions, configurable actions, and edge timing.

use client::input::{
    ContextAction, FixedDirection, InputContext, LocalInputSampler, PhysicalInput, UIAction,
    interpret_direction,
};
use client::settings::PlayerInputBindings;
use game_core::input::{GameAction, PlayerActions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Device {
    Keyboard,
    Gamepad,
}

/// The physical input a player uses for one logical action on one device.
///
/// `Left` / `Right` are fixed physical bindings and therefore never appear in
/// `PlayerInputBindings`; they still need a source so that merging two sources
/// of the same direction is observable.
fn source_for(player: usize, device: Device, action: GameAction) -> PhysicalInput {
    let name = match action {
        GameAction::Left => "Left",
        GameAction::Right => "Right",
        GameAction::SoftDrop => "SoftDrop",
        GameAction::HardDrop => "HardDrop",
        GameAction::RotateClockwise => "RotateCw",
        GameAction::RotateCounterClockwise => "RotateCcw",
    };
    let code = format!("P{player}{name}");
    match device {
        Device::Keyboard => PhysicalInput::keyboard(code),
        Device::Gamepad => PhysicalInput::gamepad(code),
    }
}

/// Bindings that map each configurable action to a keyboard *and* a gamepad input.
fn bindings_for(player: usize) -> PlayerInputBindings {
    let mut bindings = PlayerInputBindings::default();
    for action in GameAction::CONFIGURABLE {
        bindings.bindings.insert(
            action,
            vec![
                source_for(player, Device::Keyboard, action),
                source_for(player, Device::Gamepad, action),
            ],
        );
    }
    bindings
}

fn sampler_for(players: usize) -> LocalInputSampler {
    LocalInputSampler::new((0..players).map(bindings_for).collect())
}

fn fixed_direction_of(action: GameAction) -> Option<FixedDirection> {
    match action {
        GameAction::Left => Some(FixedDirection::Left),
        GameAction::Right => Some(FixedDirection::Right),
        _ => None,
    }
}

fn press_action(
    sampler: &mut LocalInputSampler,
    player: usize,
    device: Device,
    action: GameAction,
) {
    let source = source_for(player, device, action);
    match fixed_direction_of(action) {
        Some(direction) => sampler.press_fixed_direction(player, source, direction),
        None => sampler.press(player, source),
    }
}

fn release_action(
    sampler: &mut LocalInputSampler,
    player: usize,
    device: Device,
    action: GameAction,
) {
    let source = source_for(player, device, action);
    match fixed_direction_of(action) {
        Some(direction) => sampler.release_fixed_direction(player, &source, direction),
        None => sampler.release(player, &source),
    }
}

/// component/client-input::TC-001 — one case per device/action row.
macro_rules! confirmed_input_cases {
    ($($name:ident => ($device:expr, $action:expr)),+ $(,)?) => {
        $(
            // component/client-input::TC-001
            #[test]
            fn $name() {
                let mut sampler = sampler_for(1);

                press_action(&mut sampler, 0, $device, $action);
                let sampled = sampler.sample_fixed();

                assert_eq!(
                    sampled[0],
                    PlayerActions::from_action($action),
                    "a confirmed physical input must produce exactly its logical action"
                );
            }
        )+
    };
}

confirmed_input_cases! {
    keyboard_left_is_sampled => (Device::Keyboard, GameAction::Left),
    keyboard_right_is_sampled => (Device::Keyboard, GameAction::Right),
    keyboard_soft_drop_is_sampled => (Device::Keyboard, GameAction::SoftDrop),
    keyboard_hard_drop_is_sampled => (Device::Keyboard, GameAction::HardDrop),
    keyboard_rotate_clockwise_is_sampled => (Device::Keyboard, GameAction::RotateClockwise),
    keyboard_rotate_counter_clockwise_is_sampled =>
        (Device::Keyboard, GameAction::RotateCounterClockwise),
    gamepad_left_is_sampled => (Device::Gamepad, GameAction::Left),
    gamepad_right_is_sampled => (Device::Gamepad, GameAction::Right),
    gamepad_soft_drop_is_sampled => (Device::Gamepad, GameAction::SoftDrop),
    gamepad_hard_drop_is_sampled => (Device::Gamepad, GameAction::HardDrop),
    gamepad_rotate_clockwise_is_sampled => (Device::Gamepad, GameAction::RotateClockwise),
    gamepad_rotate_counter_clockwise_is_sampled =>
        (Device::Gamepad, GameAction::RotateCounterClockwise),
}

// component/client-input::TC-001
#[test]
fn an_unmapped_keyboard_input_produces_no_logical_action() {
    let mut sampler = sampler_for(1);

    sampler.press(0, PhysicalInput::keyboard("UnboundKey"));
    let sampled = sampler.sample_fixed();

    assert_eq!(sampled[0], PlayerActions::EMPTY);
}

// component/client-input::TC-001
#[test]
fn an_unmapped_gamepad_input_produces_no_logical_action() {
    let mut sampler = sampler_for(1);

    sampler.press(0, PhysicalInput::gamepad("UnboundButton"));
    let sampled = sampler.sample_fixed();

    assert_eq!(sampled[0], PlayerActions::EMPTY);
}

// component/client-input::TC-001
#[test]
fn horizontal_directions_never_go_through_configurable_bindings() {
    let bindings = bindings_for(0);

    for fixed in [GameAction::Left, GameAction::Right] {
        assert!(
            !bindings.bindings.contains_key(&fixed),
            "{fixed:?} is a fixed physical binding, not a configurable one"
        );
    }
}

// component/client-input::TC-002
#[test]
fn only_player_one_pressing_left_fills_only_slot_zero() {
    let mut sampler = sampler_for(2);

    press_action(&mut sampler, 0, Device::Keyboard, GameAction::Left);
    let sampled = sampler.sample_fixed();

    assert!(sampled[0].contains(GameAction::Left));
    assert_eq!(sampled[1], PlayerActions::EMPTY);
}

// component/client-input::TC-002
#[test]
fn only_player_two_pressing_left_fills_only_slot_one() {
    let mut sampler = sampler_for(2);

    press_action(&mut sampler, 1, Device::Keyboard, GameAction::Left);
    let sampled = sampler.sample_fixed();

    assert_eq!(sampled[0], PlayerActions::EMPTY);
    assert!(sampled[1].contains(GameAction::Left));
}

// component/client-input::TC-002
#[test]
fn both_players_pressing_left_fill_their_own_slots() {
    let mut sampler = sampler_for(2);

    press_action(&mut sampler, 0, Device::Keyboard, GameAction::Left);
    press_action(&mut sampler, 1, Device::Keyboard, GameAction::Left);
    let sampled = sampler.sample_fixed();

    assert!(sampled[0].contains(GameAction::Left));
    assert!(sampled[1].contains(GameAction::Left));
}

// component/client-input::TC-002
#[test]
fn neither_player_pressing_leaves_both_slots_empty() {
    let mut sampler = sampler_for(2);

    let sampled = sampler.sample_fixed();

    assert_eq!(sampled, vec![PlayerActions::EMPTY, PlayerActions::EMPTY]);
}

// component/client-input::TC-003
#[test]
fn two_physical_sources_of_the_same_direction_merge_into_one_action() {
    let mut sampler = sampler_for(1);

    // Two bits of one device: the d-pad and the stick both mean left, and a
    // player is only ever driven by one device at a time, so this is where
    // merging actually happens.
    sampler.press_fixed_direction(0, PhysicalInput::gamepad("DPadLeft"), FixedDirection::Left);
    sampler.press_fixed_direction(
        0,
        PhysicalInput::gamepad("LeftStickX"),
        FixedDirection::Left,
    );
    let sampled = sampler.sample_fixed();

    assert_eq!(
        sampled[0],
        PlayerActions::from_action(GameAction::Left),
        "two sources of the same direction are not a conflict and must not duplicate"
    );
}

/// component/client-input::TC-004 — one case per continuous action and timing.
macro_rules! continuous_action_cases {
    ($($held:ident, $between:ident, $released:ident => $action:expr);+ $(;)?) => {
        $(
            // component/client-input::TC-004
            #[test]
            fn $held() {
                let mut sampler = sampler_for(1);
                press_action(&mut sampler, 0, Device::Keyboard, $action);

                for tick in 0..3 {
                    let sampled = sampler.sample_fixed();
                    assert!(
                        sampled[0].contains($action),
                        "tick {tick} must sample the still-pressed action"
                    );
                }
            }

            // component/client-input::TC-004
            #[test]
            fn $between() {
                let mut sampler = sampler_for(1);
                sampler.sample_fixed();

                press_action(&mut sampler, 0, Device::Keyboard, $action);
                release_action(&mut sampler, 0, Device::Keyboard, $action);
                let sampled = sampler.sample_fixed();

                assert!(
                    !sampled[0].contains($action),
                    "a press that ends between ticks produces no continuous input"
                );
            }

            // component/client-input::TC-004
            #[test]
            fn $released() {
                let mut sampler = sampler_for(1);
                press_action(&mut sampler, 0, Device::Keyboard, $action);
                assert!(sampler.sample_fixed()[0].contains($action));

                release_action(&mut sampler, 0, Device::Keyboard, $action);
                let sampled = sampler.sample_fixed();

                assert!(
                    !sampled[0].contains($action),
                    "releasing before the boundary drops the action on the next tick"
                );
            }
        )+
    };
}

continuous_action_cases! {
    left_held_across_three_ticks,
    left_pressed_and_released_between_ticks,
    left_released_before_the_next_tick => GameAction::Left;

    right_held_across_three_ticks,
    right_pressed_and_released_between_ticks,
    right_released_before_the_next_tick => GameAction::Right;

    soft_drop_held_across_three_ticks,
    soft_drop_pressed_and_released_between_ticks,
    soft_drop_released_before_the_next_tick => GameAction::SoftDrop;
}

/// component/client-input::TC-005 — one case per edge action and timing.
macro_rules! edge_action_cases {
    ($($between:ident, $held:ident, $repeated:ident => $action:expr);+ $(;)?) => {
        $(
            // component/client-input::TC-005
            #[test]
            fn $between() {
                let mut sampler = sampler_for(1);
                sampler.sample_fixed();

                press_action(&mut sampler, 0, Device::Keyboard, $action);
                release_action(&mut sampler, 0, Device::Keyboard, $action);
                let sampled = sampler.sample_fixed();

                assert!(
                    sampled[0].contains($action),
                    "a press completed between ticks still commits on the next tick"
                );
                assert!(
                    !sampler.sample_fixed()[0].contains($action),
                    "the same press must not commit twice"
                );
            }

            // component/client-input::TC-005
            #[test]
            fn $held() {
                let mut sampler = sampler_for(1);
                press_action(&mut sampler, 0, Device::Keyboard, $action);

                assert!(
                    sampler.sample_fixed()[0].contains($action),
                    "the press edge commits on the nearest following tick"
                );
                for tick in 1..3 {
                    assert!(
                        !sampler.sample_fixed()[0].contains($action),
                        "tick {tick} must not repeat a held one-shot action"
                    );
                }
            }

            // component/client-input::TC-005
            #[test]
            fn $repeated() {
                let mut sampler = sampler_for(1);
                press_action(&mut sampler, 0, Device::Keyboard, $action);
                assert!(sampler.sample_fixed()[0].contains($action));
                release_action(&mut sampler, 0, Device::Keyboard, $action);
                assert!(!sampler.sample_fixed()[0].contains($action));

                press_action(&mut sampler, 0, Device::Keyboard, $action);
                let sampled = sampler.sample_fixed();

                assert!(
                    sampled[0].contains($action),
                    "a second press edge produces a second action"
                );
            }
        )+
    };
}

edge_action_cases! {
    hard_drop_pressed_between_ticks,
    hard_drop_held_across_three_ticks,
    hard_drop_pressed_again_after_release => GameAction::HardDrop;

    rotate_clockwise_pressed_between_ticks,
    rotate_clockwise_held_across_three_ticks,
    rotate_clockwise_pressed_again_after_release => GameAction::RotateClockwise;

    rotate_counter_clockwise_pressed_between_ticks,
    rotate_counter_clockwise_held_across_three_ticks,
    rotate_counter_clockwise_pressed_again_after_release => GameAction::RotateCounterClockwise;
}

// component/client-input::TC-006
#[test]
fn the_same_direction_produces_a_game_action_in_gameplay_context() {
    let action = interpret_direction(FixedDirection::Left, InputContext::Gameplay);

    assert_eq!(action, Some(ContextAction::Game(GameAction::Left)));
}

// component/client-input::TC-006
#[test]
fn the_same_direction_produces_a_ui_action_in_menu_context() {
    let action = interpret_direction(FixedDirection::Left, InputContext::Menu);

    assert_eq!(action, Some(ContextAction::Ui(UIAction::Left)));
}

// component/client-input::TC-006
#[test]
fn gameplay_and_menu_directions_stay_in_separate_domains() {
    let gameplay = interpret_direction(FixedDirection::Left, InputContext::Gameplay)
        .expect("gameplay interprets the left direction");
    let menu = interpret_direction(FixedDirection::Left, InputContext::Menu)
        .expect("menu interprets the left direction");

    assert_ne!(
        gameplay, menu,
        "one physical direction yields two independent domain actions"
    );

    let mut sampler = sampler_for(1);
    sampler.press_fixed_direction(
        0,
        PhysicalInput::keyboard("ArrowLeft"),
        FixedDirection::Left,
    );
    let sampled = sampler.sample_fixed();

    assert_eq!(
        sampled[0],
        PlayerActions::from_action(GameAction::Left),
        "only the rules-domain action reaches the tick input container"
    );
}
