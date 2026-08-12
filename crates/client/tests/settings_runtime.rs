//! The settings save request and the runtime application of a changed setting.

mod common;

use bevy::prelude::*;
use client::bootstrap::BootstrapPaths;
use client::i18n::Localization;
use client::settings::{
    LastSaveError, SaveSettingsRequest, SettingsStore, UserSettings, WindowModeSetting,
};
use common::controlled_app;

/// An app whose settings file lives in a temporary directory.
fn app_with_settings_path() -> (App, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temporary settings dir");
    let mut app = controlled_app();
    app.insert_resource(BootstrapPaths {
        settings: Some(dir.path().join("settings.ron")),
    });
    (app, dir)
}

// docs/test/game-infrastructure.md TC-004
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

// docs/test/game-infrastructure.md TC-008
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
