//! The startup barrier: when it releases, and that fallbacks still release it.

mod common;

use bevy::prelude::*;
use client::app_state::{AppState, AppTransitionCause, AppTransitionRequests};
use client::bootstrap::{
    BootstrapDiagnostics, BootstrapPaths, BootstrapStatus, BootstrapTaskState, request_main_menu,
};
use client::i18n::Localization;
use client::settings::UserSettings;
use common::{
    controlled_app_with_asset_root, current_state, run_until_bootstrap_ready, state_only_app,
};

/// Run only the barrier coordination system and report what it requested.
fn pending_requests_for(
    settings: BootstrapTaskState,
    localization: BootstrapTaskState,
) -> Vec<(AppState, AppTransitionCause)> {
    let mut app = state_only_app();
    app.insert_resource(BootstrapStatus {
        settings,
        localization,
    });

    app.world_mut()
        .run_system_cached(request_main_menu)
        .expect("the coordination system runs");

    app.world()
        .resource::<AppTransitionRequests>()
        .pending
        .iter()
        .map(|request| (request.target, request.cause))
        .collect()
}

// docs/test/game-infrastructure.md TC-046
#[test]
fn both_tasks_pending_keeps_boot() {
    assert_eq!(
        pending_requests_for(BootstrapTaskState::Pending, BootstrapTaskState::Pending),
        vec![]
    );
}

// docs/test/game-infrastructure.md TC-046
#[test]
fn only_localization_resolved_keeps_boot() {
    assert_eq!(
        pending_requests_for(BootstrapTaskState::Pending, BootstrapTaskState::Resolved),
        vec![]
    );
}

// docs/test/game-infrastructure.md TC-046
#[test]
fn only_settings_resolved_keeps_boot() {
    assert_eq!(
        pending_requests_for(BootstrapTaskState::Resolved, BootstrapTaskState::Pending),
        vec![]
    );
}

// docs/test/game-infrastructure.md TC-046
#[test]
fn both_tasks_resolved_requests_main_menu_exactly_once() {
    assert_eq!(
        pending_requests_for(BootstrapTaskState::Resolved, BootstrapTaskState::Resolved),
        vec![(AppState::MainMenu, AppTransitionCause::BootstrapReady)]
    );
}

// docs/test/game-infrastructure.md TC-046
#[test]
fn a_resolved_barrier_does_not_re_request_on_later_frames() {
    let mut app = state_only_app();
    app.insert_resource(BootstrapStatus {
        settings: BootstrapTaskState::Resolved,
        localization: BootstrapTaskState::Resolved,
    });

    for _ in 0..3 {
        app.world_mut()
            .run_system_cached(request_main_menu)
            .expect("the coordination system runs");
    }

    assert_eq!(
        app.world()
            .resource::<AppTransitionRequests>()
            .pending
            .len(),
        1,
        "the barrier releases with a single request"
    );
}

/// An asset root and settings file where every startup load fails.
fn failing_bootstrap_app() -> (App, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("temporary bootstrap root");
    let i18n = root.path().join("i18n");
    std::fs::create_dir_all(&i18n).expect("create i18n dir");

    let settings_path = root.path().join("settings.ron");
    std::fs::write(&settings_path, "(").expect("write malformed settings");
    // en.json is never created (missing); zh-CN.json carries an unsupported schema.
    std::fs::write(
        i18n.join("zh-CN.json"),
        r#"{ "schema_version": 255, "locale": "zh-CN", "messages": {} }"#,
    )
    .expect("write unsupported catalog");

    let mut app = controlled_app_with_asset_root(root.path().to_string_lossy().into_owned());
    app.insert_resource(BootstrapPaths {
        settings: Some(settings_path),
    });

    (app, root)
}

// docs/test/game-infrastructure.md TC-047
#[test]
fn failed_settings_and_catalog_loads_still_release_the_barrier() {
    let (mut app, _root) = failing_bootstrap_app();

    run_until_bootstrap_ready(&mut app);

    let status = *app.world().resource::<BootstrapStatus>();
    assert_eq!(status.settings, BootstrapTaskState::Resolved);
    assert_eq!(status.localization, BootstrapTaskState::Resolved);
    assert!(status.is_ready());
}

// docs/test/game-infrastructure.md TC-047
#[test]
fn a_fallback_bootstrap_keeps_its_diagnostics_and_usable_values() {
    let (mut app, _root) = failing_bootstrap_app();

    run_until_bootstrap_ready(&mut app);

    let diagnostics = app.world().resource::<BootstrapDiagnostics>();
    assert!(
        diagnostics.settings.is_some(),
        "the malformed settings file must leave a diagnostic"
    );
    assert_eq!(
        diagnostics.localization.len(),
        2,
        "both the missing and the unsupported catalog must leave diagnostics"
    );

    assert_eq!(
        app.world().resource::<UserSettings>(),
        &UserSettings::default(),
        "settings fall back to complete built-in defaults"
    );
    assert_eq!(
        app.world()
            .resource::<Localization>()
            .text("main_menu.start"),
        "Start",
        "text queries stay safe after a catalog fallback"
    );
}

// docs/test/game-infrastructure.md TC-047
#[test]
fn a_fallback_bootstrap_still_reaches_main_menu() {
    let (mut app, _root) = failing_bootstrap_app();

    run_until_bootstrap_ready(&mut app);
    app.update();
    app.update();

    assert_eq!(current_state(&app), AppState::MainMenu);
}
