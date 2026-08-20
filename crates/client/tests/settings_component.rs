//! Default values, persistence round-trip, and configurable-binding scope of `UserSettings`.

use client::input::{PhysicalInput, UIAction};
use client::settings::{
    AnimationIntensity, BindingCapture, BindingConflict, BindingOwner, CaptureOutcome,
    DeviceCategory, PlayerInputBindings, SETTINGS_SCHEMA_VERSION, SettingsError, SettingsLoad,
    SettingsStore, UserSettings, WindowModeSetting, parse_settings, serialize_settings,
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
    // Off by default: matching is decided by colour, so the extra symbol is a
    // redundancy the player opts into rather than the standard board.
    assert!(!settings.color_assist);
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
    animation_intensity: Reduced,
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
    assert_eq!(settings.animation_intensity, AnimationIntensity::Reduced);
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

/// The free key these cases hand around: bound to nothing, claimed by no fixed
/// binding.
fn free_key() -> PhysicalInput {
    PhysicalInput::keyboard("KeyX")
}

/// Player settings where the free key is bound to `SoftDrop` for player 0 only.
fn settings_with_free_key_on_soft_drop() -> UserSettings {
    let mut settings = UserSettings::default();
    settings.players[0]
        .bindings
        .insert(GameAction::SoftDrop, vec![free_key()]);
    settings
}

// component/user-settings::TC-006
#[test]
fn rebinding_an_occupied_input_to_hard_drop_reports_a_named_conflict() {
    let settings = settings_with_free_key_on_soft_drop();
    let before = settings.clone();

    let conflict = settings
        .binding_conflict(0, GameAction::HardDrop, &free_key())
        .expect("the key is already taken by SoftDrop");

    assert_eq!(conflict.requested, GameAction::HardDrop);
    assert_eq!(
        conflict.existing,
        BindingOwner::Player {
            player: 0,
            action: GameAction::SoftDrop,
        }
    );
    assert_eq!(conflict.input, free_key());
    assert_eq!(
        settings, before,
        "querying a conflict must not overwrite settings before the UI decides"
    );
}

// component/user-settings::TC-006
#[test]
fn rebinding_an_occupied_input_to_rotate_clockwise_reports_a_named_conflict() {
    let settings = settings_with_free_key_on_soft_drop();

    let conflict = settings
        .binding_conflict(0, GameAction::RotateClockwise, &free_key())
        .expect("the key is already taken by SoftDrop");

    assert_eq!(conflict.requested, GameAction::RotateClockwise);
    assert_eq!(
        conflict.existing,
        BindingOwner::Player {
            player: 0,
            action: GameAction::SoftDrop,
        }
    );
}

// component/user-settings::TC-006
#[test]
fn the_other_player_may_not_reuse_a_key_but_may_reuse_a_pad_button() {
    let settings = settings_with_free_key_on_soft_drop();

    let key = settings
        .binding_conflict(1, GameAction::SoftDrop, &free_key())
        .expect("one keyboard serves both locals, so the key is taken");
    assert_eq!(
        key.existing,
        BindingOwner::Player {
            player: 0,
            action: GameAction::SoftDrop,
        },
        "the refusal has to name the player holding the key, not just the action"
    );

    // Both players default to the same pad column on purpose: each holds their
    // own pad, so the same button is not the same physical input.
    let held_by_player_0 = settings.players[0]
        .input_for(GameAction::RotateClockwise, DeviceCategory::Gamepad)
        .expect("the default pad column is populated")
        .clone();
    assert!(
        settings
            .binding_conflict(1, GameAction::RotateClockwise, &held_by_player_0)
            .is_none(),
        "a pad button is only claimed inside one player's own map"
    );
}

// component/user-settings::TC-006
#[test]
fn rebinding_an_action_to_the_input_it_already_holds_is_not_a_conflict() {
    let settings = settings_with_free_key_on_soft_drop();

    assert!(
        settings
            .binding_conflict(0, GameAction::SoftDrop, &free_key())
            .is_none(),
        "confirming the key an action already has must not read as a collision"
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
    let settings = settings_with_free_key_on_soft_drop();

    for fixed in FIXED_GAME_ACTIONS {
        assert!(
            settings.binding_conflict(0, fixed, &free_key()).is_none(),
            "{fixed:?} is fixed and must stay outside the conflict-detection scope"
        );
    }
    assert!(
        settings
            .binding_conflict(0, GameAction::HardDrop, &free_key())
            .is_some(),
        "the configurable scope itself still reports conflicts"
    );
}

// component/user-settings::TC-011
#[test]
fn a_physical_input_a_fixed_binding_claims_is_refused_by_name() {
    let settings = UserSettings::default();

    // A horizontal direction moves the falling group for the whole match, so no
    // configurable action may take it.
    for action in GameAction::CONFIGURABLE {
        let conflict = settings
            .binding_conflict(0, action, &PhysicalInput::keyboard("KeyA"))
            .unwrap_or_else(|| panic!("{action:?} must not be allowed to take a fixed direction"));
        assert_eq!(conflict.existing, BindingOwner::Fixed);
    }

    // Pause is live in every context, on either device.
    for input in [
        PhysicalInput::keyboard("Escape"),
        PhysicalInput::gamepad("Start"),
    ] {
        assert_eq!(
            settings
                .binding_conflict(0, GameAction::SoftDrop, &input)
                .map(|conflict| conflict.existing),
            Some(BindingOwner::Fixed),
            "{input:?} proposes a pause and cannot also be a rules action"
        );
    }

    // The vertical directions only move menu focus, which is the room the
    // default drop bindings sit in -- but the rotations also carry confirm and
    // back, so for those two the same key is taken.
    let up = PhysicalInput::keyboard("KeyW");
    assert!(
        settings
            .binding_conflict(1, GameAction::SoftDrop, &up)
            .is_some_and(|conflict| conflict.existing
                == BindingOwner::Player {
                    player: 0,
                    action: GameAction::HardDrop,
                }),
        "P1 already holds the key; that is a player conflict, not a fixed one"
    );
    assert_eq!(
        UserSettings {
            players: std::array::from_fn(|_| PlayerInputBindings {
                bindings: Default::default(),
            }),
            ..UserSettings::default()
        }
        .binding_conflict(0, GameAction::RotateCounterClockwise, &up)
        .map(|conflict| conflict.existing),
        Some(BindingOwner::Fixed),
        "the rotations carry the menu confirm and back, which the key already moves focus for"
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

// component/user-settings::TC-009
#[test]
fn schema_evolution_defaults_a_new_field_ignores_an_unknown_one_and_resets_on_a_version_bump() {
    // A field added since the file was written: only that field falls back,
    // every other choice the player made survives.
    let missing_field = r#"(
    schema_version: 1,
    language: "zh-CN",
    vibration: false,
)"#;
    let parsed = parse_settings(missing_field).expect("a missing field is not a parse failure");
    assert_eq!(parsed.animation_intensity, AnimationIntensity::Full);
    assert_eq!(parsed.language, "zh-CN");
    assert!(!parsed.vibration);

    // A field this build no longer knows: ignored, the rest still applies.
    let unknown_field = r#"(
    schema_version: 1,
    language: "zh-CN",
    colour_blind_assist: true,
)"#;
    let parsed = parse_settings(unknown_field).expect("an unknown field is not a parse failure");
    assert_eq!(parsed.language, "zh-CN");

    // An incompatible change: the whole document is abandoned for defaults.
    let bumped = r#"(
    schema_version: 255,
    language: "zh-CN",
)"#;
    let error = parse_settings(bumped).expect_err("an unsupported schema cannot be trusted");
    assert!(matches!(
        error,
        SettingsError::UnsupportedSchema {
            found: 255,
            supported: SETTINGS_SCHEMA_VERSION
        }
    ));
}

// component/user-settings::TC-010
#[test]
fn a_capture_writes_cancels_or_reports_a_conflict_for_the_page_to_resolve() {
    let free_key = PhysicalInput::keyboard("KeyP");
    let taken_key = PhysicalInput::keyboard("KeyW");

    // Writing: an unclaimed input binds straight away.
    let mut settings = UserSettings::default();
    let capture = BindingCapture::open(0, GameAction::SoftDrop, DeviceCategory::Keyboard)
        .expect("SoftDrop is configurable");
    let outcome = capture
        .offer(&mut settings, &free_key)
        .expect("player 0 exists");
    assert_eq!(outcome, CaptureOutcome::Bound);
    assert!(
        settings.players[0]
            .actions_for(&free_key)
            .eq([GameAction::SoftDrop])
    );

    // Another device category is not this capture's business, and leaves it open.
    let mut settings = UserSettings::default();
    let outcome = capture
        .offer(&mut settings, &PhysicalInput::gamepad("North"))
        .expect("player 0 exists");
    assert_eq!(outcome, CaptureOutcome::Ignored);
    assert_eq!(settings, UserSettings::default());

    // Cancelling: a back input ends the capture and keeps the original binding.
    let mut settings = UserSettings::default();
    let cancelled = BindingCapture::open(0, GameAction::SoftDrop, DeviceCategory::Keyboard)
        .expect("SoftDrop is configurable");
    assert_eq!(cancelled.cancel(), CaptureOutcome::Cancelled);
    assert_eq!(settings, UserSettings::default());

    // Conflicting: reported, and the table is left untouched until the page decides.
    let capture = BindingCapture::open(0, GameAction::SoftDrop, DeviceCategory::Keyboard)
        .expect("SoftDrop is configurable");
    let outcome = capture
        .offer(&mut settings, &taken_key)
        .expect("player 0 exists");
    assert_eq!(
        outcome,
        CaptureOutcome::Conflict(BindingConflict {
            requested: GameAction::SoftDrop,
            existing: BindingOwner::Player {
                player: 0,
                action: GameAction::HardDrop,
            },
            input: taken_key.clone(),
        })
    );
    assert_eq!(settings, UserSettings::default());

    // Refusal is final: offering the same taken input again reports the same
    // conflict rather than eventually giving in, and the previous owner keeps
    // it. Taking it away would leave `HardDrop` -- and, for a rotation, the
    // menu action riding on it -- with nothing bound.
    let again = capture
        .offer(&mut settings, &taken_key)
        .expect("player 0 exists");
    assert_eq!(again, outcome);
    assert_eq!(settings, UserSettings::default());
    assert!(
        settings.players[0]
            .actions_for(&taken_key)
            .eq([GameAction::HardDrop])
    );

    // A fixed binding never opens a capture at all.
    assert!(BindingCapture::open(0, GameAction::Left, DeviceCategory::Keyboard).is_none());
}

// component/user-settings::TC-012
#[test]
fn binding_replaces_what_the_action_held_on_that_device() {
    let mut bindings = PlayerInputBindings::for_player(0);
    let before_pad = bindings
        .input_for(GameAction::SoftDrop, DeviceCategory::Gamepad)
        .expect("the default pad column is populated")
        .clone();

    bindings.bind(GameAction::SoftDrop, PhysicalInput::keyboard("KeyX"));
    bindings.bind(GameAction::SoftDrop, PhysicalInput::keyboard("KeyZ"));

    assert_eq!(
        bindings.bindings[&GameAction::SoftDrop]
            .iter()
            .filter(|input| input.category() == DeviceCategory::Keyboard)
            .count(),
        1,
        "a second keyboard binding would fire while the page showed the first"
    );
    assert_eq!(
        bindings.input_for(GameAction::SoftDrop, DeviceCategory::Keyboard),
        Some(&PhysicalInput::keyboard("KeyZ"))
    );
    assert_eq!(
        bindings.input_for(GameAction::SoftDrop, DeviceCategory::Gamepad),
        Some(&before_pad),
        "the other device category is untouched"
    );
}

// component/user-settings::TC-013
#[test]
fn loading_repairs_a_document_that_breaks_the_binding_invariants() {
    // Written the way the pre-fix capture wrote them: every accepted press
    // appended, nothing checked the other player, nothing checked the fixed
    // bindings. Only the first entry of each category was ever visible.
    let document = r#"(
    schema_version: 1,
    players: ((
        bindings: {
            SoftDrop: [Keyboard("KeyS"), Gamepad("DPadDown")],
            HardDrop: [Keyboard("KeyW"), Gamepad("DPadUp")],
            RotateClockwise: [Keyboard("KeyK"), Gamepad("East")],
            RotateCounterClockwise: [Keyboard("KeyJ"), Gamepad("South")],
        },
    ), (
        bindings: {
            SoftDrop: [Keyboard("ArrowDown"), Gamepad("DPadDown")],
            HardDrop: [Keyboard("ArrowUp"), Gamepad("DPadUp")],
            RotateClockwise: [Keyboard("Numpad2"), Gamepad("East"), Keyboard("Semicolon"), Keyboard("KeyS"), Keyboard("BracketLeft")],
            RotateCounterClockwise: [Keyboard("Numpad1"), Gamepad("South"), Keyboard("KeyW")],
        },
    )),
)"#;
    let mut settings = parse_settings(document).expect("the document is well-formed RON");

    let dropped = settings.normalize_bindings();

    let dropped_inputs: Vec<&PhysicalInput> = dropped.iter().map(|entry| &entry.input).collect();
    assert_eq!(
        dropped_inputs,
        vec![
            &PhysicalInput::keyboard("Semicolon"),
            &PhysicalInput::keyboard("KeyS"),
            &PhysicalInput::keyboard("BracketLeft"),
            &PhysicalInput::keyboard("KeyW"),
        ],
        "the first binding of each category survives, because that is the one the page has been showing"
    );
    assert!(dropped.iter().all(|entry| entry.player == 1));
    assert_eq!(
        settings.players[1].input_for(GameAction::RotateClockwise, DeviceCategory::Keyboard),
        Some(&PhysicalInput::keyboard("Numpad2"))
    );
    assert!(
        settings.players[1]
            .actions_for(&PhysicalInput::keyboard("KeyS"))
            .next()
            .is_none(),
        "P1's soft drop key must stop acting as P2's menu back"
    );

    // The repaired document is stable: running it again drops nothing.
    assert!(settings.normalize_bindings().is_empty());
    // And the built-in defaults were never in violation to begin with.
    assert!(UserSettings::default().normalize_bindings().is_empty());
}
