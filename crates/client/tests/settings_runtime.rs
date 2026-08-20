//! The settings save request and the runtime application of a changed setting.

mod common;

use bevy::prelude::*;
use client::bootstrap::BootstrapPaths;
use client::i18n::Localization;
use client::input::{LocalInputSampler, PhysicalInput};
use client::settings::{
    AnimationIntensity, LastSaveError, SaveSettingsRequest, SettingsStore, UserSettings,
    WindowModeSetting,
};
use common::{controlled_app, run_until_bootstrap_ready};
use game_core::input::GameAction;

/// An app whose settings file lives in a temporary directory.
fn app_with_settings_path() -> (App, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temporary settings dir");
    let mut app = controlled_app();
    app.insert_resource(BootstrapPaths {
        settings: Some(dir.path().join("settings.ron")),
    });
    (app, dir)
}

// component/user-settings::TC-004
#[test]
fn a_save_request_writes_the_current_settings_to_disk() {
    let (mut app, dir) = app_with_settings_path();
    app.update();

    app.world_mut().resource_mut::<UserSettings>().window_mode = WindowModeSetting::Fullscreen;
    app.world_mut().write_message(SaveSettingsRequest);
    app.update();

    assert!(
        app.world().resource::<LastSaveError>().0.is_none(),
        "the save must succeed: {:?}",
        app.world().resource::<LastSaveError>().0
    );

    let restored = SettingsStore::new(dir.path().join("settings.ron"))
        .load()
        .settings()
        .clone();
    assert_eq!(
        restored.window_mode,
        WindowModeSetting::Fullscreen,
        "the written file must restore the edited value"
    );
}

#[test]
fn a_failed_save_is_reported_without_touching_the_in_memory_value() {
    let (mut app, dir) = app_with_settings_path();
    app.update();

    // A directory where the settings file belongs makes the replace fail.
    std::fs::create_dir_all(dir.path().join("settings.ron")).expect("occupy the settings path");

    app.world_mut().write_message(SaveSettingsRequest);
    app.update();

    assert!(
        app.world().resource::<LastSaveError>().0.is_some(),
        "a failed write must be observable"
    );
    assert_eq!(
        app.world().resource::<UserSettings>().window_mode,
        WindowModeSetting::Windowed,
        "the in-memory value survives a failed write"
    );
}

// component/localization::TC-002
#[test]
fn changing_the_language_setting_switches_the_current_catalog() {
    let mut app = controlled_app();
    common::run_until_bootstrap_ready(&mut app);

    app.world_mut().resource_mut::<UserSettings>().language = "zh-CN".into();
    app.update();

    assert_eq!(
        app.world().resource::<Localization>().current_locale,
        "zh-CN",
        "an available locale becomes current"
    );
}

#[test]
fn an_unavailable_language_falls_back_to_the_default_locale() {
    let mut app = controlled_app();
    common::run_until_bootstrap_ready(&mut app);

    app.world_mut().resource_mut::<UserSettings>().language = "fr".into();
    app.update();

    assert_eq!(
        app.world().resource::<Localization>().current_locale,
        "en",
        "an unavailable locale must not leave the resource pointing at a missing catalog"
    );
}

// integration-system/runtime-data::TC-004
#[test]
fn every_changed_setting_reaches_its_consumer_without_a_restart() {
    let (mut app, _dir) = app_with_settings_path();
    run_until_bootstrap_ready(&mut app);

    // A window exists only so the window-mode consumer has something to write.
    let window = app.world_mut().spawn(Window::default()).id();

    {
        let mut settings = app.world_mut().resource_mut::<UserSettings>();
        settings.language = "zh-CN".into();
        settings.window_mode = WindowModeSetting::Fullscreen;
        settings.master_volume = 0.3;
        settings.animation_intensity = AnimationIntensity::Reduced;
        settings.vibration = false;
        settings.players[0].bind(GameAction::SoftDrop, PhysicalInput::keyboard("KeyP"));
    }
    app.update();

    // Language: the catalog behind text lookups actually moved.
    assert_eq!(
        app.world().resource::<Localization>().current_locale,
        "zh-CN",
        "a language change must reach the localization runtime"
    );

    // Window mode: applied to the live window, not just stored.
    assert!(
        matches!(
            app.world().get::<Window>(window).expect("window").mode,
            bevy::window::WindowMode::Fullscreen(..)
        ),
        "a window-mode change must reach the window"
    );

    // Bindings: the sampler is re-installed, so the new key produces the action
    // on the very next sampled frame rather than after leaving the page.
    assert!(
        app.world()
            .resource::<LocalInputSampler>()
            .bindings
            .first()
            .expect("player 0 bindings")
            .actions_for(&PhysicalInput::keyboard("KeyP"))
            .any(|action| action == GameAction::SoftDrop),
        "a rebinding must reach the runtime sampler"
    );

    // Volume, animation intensity and vibration are read from the settings
    // resource by their consumers; the contract is that the change is visible
    // there immediately, with no separate commit step.
    let settings = app.world().resource::<UserSettings>();
    assert!((settings.master_volume - 0.3).abs() < f32::EPSILON);
    assert_eq!(settings.animation_intensity, AnimationIntensity::Reduced);
    assert!(!settings.vibration);

    // A write failure must not roll back what already took effect in memory.
    app.insert_resource(BootstrapPaths {
        settings: Some(std::path::PathBuf::from("/nonexistent-root/settings.ron")),
    });
    app.world_mut().write_message(SaveSettingsRequest);
    app.update();
    assert!(
        app.world().resource::<LastSaveError>().0.is_some(),
        "the save was expected to fail"
    );
    assert_eq!(app.world().resource::<UserSettings>().language, "zh-CN");
}
