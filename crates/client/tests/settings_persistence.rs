//! Atomic settings persistence: platform path, successful reload, and replace failure.

use client::settings::{SettingsLoad, SettingsStore, UserSettings};

// docs/test/game-infrastructure.md TC-033
#[test]
fn the_official_settings_path_lives_in_the_platform_config_root() {
    let store = SettingsStore::platform_default().expect("the platform exposes a config root");
    let expected_root =
        directories::ProjectDirs::from("org", "PC-Parrot-ACGN-Club", "Codename-Psi")
            .expect("the platform exposes a config root");

    assert_eq!(store.path.parent(), Some(expected_root.config_dir()));
    assert_eq!(
        store.path.file_name().and_then(|name| name.to_str()),
        Some("settings.ron")
    );
}

// docs/test/game-infrastructure.md TC-033
#[test]
fn a_successful_save_is_reloadable_and_leaves_no_partial_file() {
    let root = tempfile::tempdir().expect("temporary config root");
    let store = SettingsStore::new(root.path().join("settings.ron"));

    let old = UserSettings {
        language: "en".into(),
        ..Default::default()
    };
    store.save(&old).expect("the first save succeeds");

    let new = UserSettings {
        language: "zh-CN".into(),
        ..old.clone()
    };
    store.save(&new).expect("the replacing save succeeds");

    let reloaded = store.load();
    assert!(matches!(reloaded, SettingsLoad::Loaded(_)));
    assert_eq!(reloaded.settings().language, "zh-CN");
    assert!(
        !root.path().join("settings.ron.tmp").is_file(),
        "no partial temporary file may survive a successful save"
    );
}

// docs/test/game-infrastructure.md TC-033
#[test]
fn a_failed_replace_keeps_the_official_file_and_the_in_memory_value() {
    let root = tempfile::tempdir().expect("temporary config root");
    let path = root.path().join("settings.ron");
    let store = SettingsStore::new(&path);

    let old = UserSettings {
        language: "en".into(),
        ..Default::default()
    };
    store.save(&old).expect("the first save succeeds");

    let in_memory = UserSettings {
        language: "zh-CN".into(),
        ..old.clone()
    };

    // Fail the replace step itself: the staging file must already be written,
    // otherwise this would only cover a failure to stage.
    let mut staged_before_replace = false;
    let error = store
        .save_with(&in_memory, |staged, _official| {
            staged_before_replace = staged.is_file();
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "replace refused",
            ))
        })
        .expect_err("the failed replace must fail observably");

    assert!(
        staged_before_replace,
        "the staging file must be fully written before the replace step runs"
    );
    assert!(
        !error.to_string().is_empty(),
        "the failure must be observable to the caller"
    );
    assert_eq!(
        in_memory.language, "zh-CN",
        "the in-memory value keeps the updated settings"
    );
    assert_eq!(
        store.load().settings().language,
        "en",
        "the official file must still hold the previous settings"
    );
}
