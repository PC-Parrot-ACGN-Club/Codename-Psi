//! Default values, persistence round-trip, and configurable-binding scope of `UserSettings`.

use client::input::{PhysicalInput, UIAction};
use client::settings::{
    SETTINGS_SCHEMA_VERSION, SettingsError, SettingsLoad, SettingsStore, UserSettings,
    WindowModeSetting, parse_settings, serialize_settings,
};
use game_core::input::GameAction;

/// The six fixed-binding UI actions, listed here so the scope assertions below
/// stay readable next to the configurable `GameAction` set.
const FIXED_UI_ACTIONS: [UIAction; 6] = [
    UIAction::Left,
    UIAction::Right,
    UIAction::Up,
    UIAction::Down,
    UIAction::Confirm,
    UIAction::Back,
];

/// `GameAction`s that are physically fixed and must never be persisted as bindings.
const FIXED_GAME_ACTIONS: [GameAction; 2] = [GameAction::Left, GameAction::Right];

fn assert_only_configurable_bindings(settings: &UserSettings) {
    for (index, player) in settings.players.iter().enumerate() {
        let keys: Vec<GameAction> = player.bindings.keys().copied().collect();
        assert_eq!(
            keys,
            GameAction::CONFIGURABLE.to_vec(),
            "player {index} must persist exactly the four configurable actions"
        );
        for fixed in FIXED_GAME_ACTIONS {
            assert!(
                !player.bindings.contains_key(&fixed),
                "player {index} must not persist a binding for fixed action {fixed:?}"
            );
        }
    }
    assert_eq!(
        FIXED_UI_ACTIONS.len(),
        6,
        "UIAction is a separate domain type and has no representation in PlayerInputBindings"
    );
}

// component/user-settings::TC-001
#[test]
fn default_settings_are_complete_and_safe() {
    let settings = UserSettings::default();

    assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
    assert_eq!(settings.language, "en");
    assert_eq!(settings.window_mode, WindowModeSetting::Windowed);
    assert_eq!(settings.master_volume, 1.0);
    assert_eq!(settings.sfx_volume, 1.0);
    assert!(settings.vibration);
    assert_only_configurable_bindings(&settings);
}

// component/user-settings::TC-002
#[test]
fn a_missing_settings_file_falls_back_to_defaults_without_an_error() {
    let root = tempfile::tempdir().expect("temporary config root");
    let store = SettingsStore::new(root.path().join("settings.ron"));

    let load = store.load();

    assert_eq!(load.settings(), &UserSettings::default());
    match load {
        SettingsLoad::Defaulted { error: None, .. } => {}
        other => panic!("a missing file must default without a diagnostic, got {other:?}"),
    }
}

// component/user-settings::TC-002
#[test]
fn a_malformed_settings_file_falls_back_to_defaults_with_a_parse_diagnostic() {
    let root = tempfile::tempdir().expect("temporary config root");
    let path = root.path().join("settings.ron");
    std::fs::write(&path, "(").expect("write malformed settings");
    let store = SettingsStore::new(&path);

    let load = store.load();

    assert_eq!(load.settings(), &UserSettings::default());
    match load {
        SettingsLoad::Defaulted {
            error: Some(SettingsError::Parse(_)),
            ..
        } => {}
        other => panic!("a malformed file must leave a parse diagnostic, got {other:?}"),
    }
}

// component/user-settings::TC-002
#[test]
fn an_unsupported_settings_schema_falls_back_to_defaults_with_a_version_diagnostic() {
    let root = tempfile::tempdir().expect("temporary config root");
    let path = root.path().join("settings.ron");
    let unsupported = UserSettings {
        schema_version: 255,
        ..Default::default()
    };
    std::fs::write(&path, serialize_settings(&unsupported).expect("serialize")).expect("write");
    let store = SettingsStore::new(&path);

    let load = store.load();

    assert_eq!(load.settings(), &UserSettings::default());
    match load {
        SettingsLoad::Defaulted {
            error: Some(SettingsError::UnsupportedSchema { found, supported }),
            ..
        } => {
            assert_eq!(found, 255);
            assert_eq!(supported, SETTINGS_SCHEMA_VERSION);
        }
        other => panic!("an unsupported schema must be distinguishable, got {other:?}"),
    }
}

/// A full non-default settings document covering every persisted field, with
/// distinguishable keyboard and gamepad bindings for both players.
const FULL_SETTINGS_RON: &str = r#"(
    schema_version: 1,
    language: "zh-CN",
    window_mode: BorderlessFullscreen,
    master_volume: 0.4,
    sfx_volume: 0.25,
    players: ((
        bindings: {
            SoftDrop: [Keyboard("KeyS"), Gamepad("P1DPadDown")],
            HardDrop: [Keyboard("KeyW"), Gamepad("P1South")],
            RotateClockwise: [Keyboard("KeyD"), Gamepad("P1East")],
            RotateCounterClockwise: [Keyboard("KeyA"), Gamepad("P1West")],
        },
    ), (
        bindings: {
            SoftDrop: [Keyboard("ArrowDown"), Gamepad("P2DPadDown")],
            HardDrop: [Keyboard("ArrowUp"), Gamepad("P2South")],
            RotateClockwise: [Keyboard("ArrowRight"), Gamepad("P2East")],
            RotateCounterClockwise: [Keyboard("ArrowLeft"), Gamepad("P2West")],
        },
    )),
    vibration: false,
)
"#;

// component/user-settings::TC-003
#[test]
fn a_supported_settings_document_restores_every_persisted_field() {
    let settings = parse_settings(FULL_SETTINGS_RON).expect("supported schema parses");

    assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
    assert_eq!(settings.language, "zh-CN");
    assert_eq!(
        settings.window_mode,
        WindowModeSetting::BorderlessFullscreen
    );
    assert_eq!(settings.master_volume, 0.4);
    assert_eq!(settings.sfx_volume, 0.25);
    assert!(!settings.vibration);
    assert_only_configurable_bindings(&settings);

    assert_eq!(
        settings.players[0].bindings[&GameAction::SoftDrop],
        vec![
            PhysicalInput::keyboard("KeyS"),
            PhysicalInput::gamepad("P1DPadDown")
        ]
    );
    assert_eq!(
        settings.players[1].bindings[&GameAction::SoftDrop],
        vec![
            PhysicalInput::keyboard("ArrowDown"),
            PhysicalInput::gamepad("P2DPadDown")
        ]
    );
    assert_ne!(
        settings.players[0], settings.players[1],
        "player data must be neither swapped nor merged"
    );
}

// component/user-settings::TC-004
#[test]
fn serializing_and_reloading_settings_preserves_every_value() {
    let original = parse_settings(FULL_SETTINGS_RON).expect("fixture parses");

    let serialized = serialize_settings(&original).expect("settings serialize");
    let restored = parse_settings(&serialized).expect("serialized settings parse");

    assert_eq!(restored, original);
    assert_eq!(restored.schema_version, SETTINGS_SCHEMA_VERSION);
    assert!(
        serialized.contains("schema_version"),
        "the persisted document must carry its schema version"
    );
    assert_only_configurable_bindings(&restored);
}

// component/user-settings::TC-005
#[test]
fn both_players_keep_independent_keyboard_and_gamepad_bindings() {
    let original = parse_settings(FULL_SETTINGS_RON).expect("fixture parses");

    let restored =
        parse_settings(&serialize_settings(&original).expect("serialize")).expect("reparse");

    assert_eq!(
        restored.players[0].bindings[&GameAction::HardDrop],
        vec![
            PhysicalInput::keyboard("KeyW"),
            PhysicalInput::gamepad("P1South")
        ]
    );
    assert_eq!(
        restored.players[1].bindings[&GameAction::HardDrop],
        vec![
            PhysicalInput::keyboard("ArrowUp"),
            PhysicalInput::gamepad("P2South")
        ]
    );

    let mut edited = restored.clone();
    edited.players[0]
        .bindings
        .insert(GameAction::SoftDrop, vec![PhysicalInput::keyboard("KeyX")]);

    assert_eq!(
        edited.players[1], restored.players[1],
        "editing P1 must not overwrite P2"
    );
    assert_ne!(edited.players[0], restored.players[0]);
    assert_only_configurable_bindings(&edited);
}

/// Player settings where `KeyA` is already bound to `SoftDrop` for player 0 only.
fn settings_with_key_a_on_soft_drop() -> UserSettings {
    let mut settings = UserSettings::default();
    settings.players[0]
        .bindings
        .insert(GameAction::SoftDrop, vec![PhysicalInput::keyboard("KeyA")]);
    settings
}

// component/user-settings::TC-006
#[test]
fn rebinding_an_occupied_input_to_hard_drop_reports_a_named_conflict() {
    let settings = settings_with_key_a_on_soft_drop();
    let before = settings.clone();

    let conflict = settings.players[0]
        .conflict(GameAction::HardDrop, &PhysicalInput::keyboard("KeyA"))
        .expect("KeyA is already taken by SoftDrop");

    assert_eq!(conflict.requested, GameAction::HardDrop);
    assert_eq!(conflict.existing, GameAction::SoftDrop);
    assert_eq!(conflict.input, PhysicalInput::keyboard("KeyA"));
    assert_eq!(
        settings, before,
        "querying a conflict must not overwrite settings before the UI decides"
    );
}

// component/user-settings::TC-006
#[test]
fn rebinding_an_occupied_input_to_rotate_clockwise_reports_a_named_conflict() {
    let settings = settings_with_key_a_on_soft_drop();

    let conflict = settings.players[0]
        .conflict(
            GameAction::RotateClockwise,
            &PhysicalInput::keyboard("KeyA"),
        )
        .expect("KeyA is already taken by SoftDrop");

    assert_eq!(conflict.requested, GameAction::RotateClockwise);
    assert_eq!(conflict.existing, GameAction::SoftDrop);
}

// component/user-settings::TC-006
#[test]
fn the_other_player_may_reuse_the_same_physical_input() {
    let settings = settings_with_key_a_on_soft_drop();

    let conflict =
        settings.players[1].conflict(GameAction::SoftDrop, &PhysicalInput::keyboard("KeyA"));

    assert!(
        conflict.is_none(),
        "conflict detection is scoped per player configuration"
    );
}

// component/user-settings::TC-007
#[test]
fn fixed_binding_actions_stay_out_of_persisted_bindings() {
    let settings = parse_settings(FULL_SETTINGS_RON).expect("fixture parses");

    assert_only_configurable_bindings(&settings);
    for fixed in FIXED_GAME_ACTIONS {
        assert!(
            !fixed.is_configurable(),
            "{fixed:?} is a fixed physical binding"
        );
    }
    for configurable in GameAction::CONFIGURABLE {
        assert!(configurable.is_configurable());
    }
}

// component/user-settings::TC-007
#[test]
fn fixed_binding_actions_are_excluded_from_conflict_detection() {
    let settings = settings_with_key_a_on_soft_drop();

    for fixed in FIXED_GAME_ACTIONS {
        assert!(
            settings.players[0]
                .conflict(fixed, &PhysicalInput::keyboard("KeyA"))
                .is_none(),
            "{fixed:?} is fixed and must stay outside the conflict-detection scope"
        );
    }
    assert!(
        settings.players[0]
            .conflict(GameAction::HardDrop, &PhysicalInput::keyboard("KeyA"))
            .is_some(),
        "the configurable scope itself still reports conflicts"
    );
}

/// A document whose P1 binding map carries one fixed-binding action alongside
/// the four configurable ones.
fn settings_ron_binding(fixed: &str) -> String {
    format!(
        r#"(
    schema_version: 1,
    players: ((
        bindings: {{
            {fixed}: [Keyboard("KeyJ")],
            SoftDrop: [Keyboard("KeyS")],
            HardDrop: [Keyboard("KeyW")],
            RotateClockwise: [Keyboard("KeyD")],
            RotateCounterClockwise: [Keyboard("KeyA")],
        }},
    ), (
        bindings: {{
            SoftDrop: [Keyboard("ArrowDown")],
            HardDrop: [Keyboard("ArrowUp")],
            RotateClockwise: [Keyboard("ArrowRight")],
            RotateCounterClockwise: [Keyboard("ArrowLeft")],
        }},
    )),
)
"#
    )
}

fn assert_fixed_binding_is_rejected(fixed: GameAction, name: &str) {
    match parse_settings(&settings_ron_binding(name)) {
        Err(SettingsError::NonConfigurableBinding(rejected)) => assert_eq!(rejected, fixed),
        other => panic!("a persisted {fixed:?} binding must be rejected, got {other:?}"),
    }
}

// component/user-settings::TC-003
#[test]
fn a_document_binding_the_fixed_left_action_is_rejected() {
    assert_fixed_binding_is_rejected(GameAction::Left, "Left");
}

// component/user-settings::TC-003
#[test]
fn a_document_binding_the_fixed_right_action_is_rejected() {
    assert_fixed_binding_is_rejected(GameAction::Right, "Right");
}

// component/user-settings::TC-004
#[test]
fn a_settings_value_carrying_a_fixed_binding_cannot_be_serialized_back() {
    for fixed in FIXED_GAME_ACTIONS {
        let mut settings = UserSettings::default();
        settings.players[0]
            .bindings
            .insert(fixed, vec![PhysicalInput::keyboard("KeyJ")]);

        match serialize_settings(&settings) {
            Err(SettingsError::NonConfigurableBinding(rejected)) => assert_eq!(rejected, fixed),
            other => panic!("a {fixed:?} binding must not be writable, got {other:?}"),
        }
    }
}

// component/user-settings::TC-007
#[test]
fn a_document_with_a_fixed_binding_defaults_and_is_never_written_back() {
    let root = tempfile::tempdir().expect("temporary config root");
    let path = root.path().join("settings.ron");
    std::fs::write(&path, settings_ron_binding("Left")).expect("write settings");
    let store = SettingsStore::new(&path);

    let load = store.load();

    assert_eq!(load.settings(), &UserSettings::default());
    match load {
        SettingsLoad::Defaulted {
            error: Some(SettingsError::NonConfigurableBinding(GameAction::Left)),
            ..
        } => {}
        other => panic!("a fixed binding must leave a scope diagnostic, got {other:?}"),
    }

    // Saving what was actually loaded must not re-emit the rejected binding.
    store
        .save(&UserSettings::default())
        .expect("the defaulted value is savable");
    let reloaded = store.load();
    assert!(matches!(reloaded, SettingsLoad::Loaded(_)));
    assert_only_configurable_bindings(reloaded.settings());
}
