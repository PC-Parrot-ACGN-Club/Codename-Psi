//! The startup barrier: when it releases, and that fallbacks still release it.

mod common;

use std::path::Path;
use std::time::Duration;

use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, AssetSourceId, PathStream, Reader,
};
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use client::GameInfrastructurePlugin;
use client::app_state::{AppState, AppTransitionCause, AppTransitionRequests};
use client::bootstrap::{
    BOOTSTRAP_TIMEOUT, BootstrapDiagnostics, BootstrapPaths, BootstrapStatus, BootstrapTaskState,
    request_main_menu,
};
use client::data::DataErrorCause;
use client::i18n::{DEFAULT_LOCALE, Localization};
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

// integration-system/application-lifecycle::TC-006
#[test]
fn both_tasks_pending_keeps_boot() {
    assert_eq!(
        pending_requests_for(BootstrapTaskState::Pending, BootstrapTaskState::Pending),
        vec![]
    );
}

// integration-system/application-lifecycle::TC-006
#[test]
fn only_localization_resolved_keeps_boot() {
    assert_eq!(
        pending_requests_for(BootstrapTaskState::Pending, BootstrapTaskState::Resolved),
        vec![]
    );
}

// integration-system/application-lifecycle::TC-006
#[test]
fn only_settings_resolved_keeps_boot() {
    assert_eq!(
        pending_requests_for(BootstrapTaskState::Resolved, BootstrapTaskState::Pending),
        vec![]
    );
}

// integration-system/application-lifecycle::TC-006
#[test]
fn both_tasks_resolved_requests_main_menu_exactly_once() {
    assert_eq!(
        pending_requests_for(BootstrapTaskState::Resolved, BootstrapTaskState::Resolved),
        vec![(AppState::MainMenu, AppTransitionCause::BootstrapReady)]
    );
}

// integration-system/application-lifecycle::TC-006
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

// integration-system/application-lifecycle::TC-007
#[test]
fn failed_settings_and_catalog_loads_still_release_the_barrier() {
    let (mut app, _root) = failing_bootstrap_app();

    run_until_bootstrap_ready(&mut app);

    let status = *app.world().resource::<BootstrapStatus>();
    assert_eq!(status.settings, BootstrapTaskState::Resolved);
    assert_eq!(status.localization, BootstrapTaskState::Resolved);
    assert!(status.is_ready());
}

// integration-system/application-lifecycle::TC-007
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

// integration-system/application-lifecycle::TC-007
#[test]
fn a_fallback_bootstrap_still_reaches_main_menu() {
    let (mut app, _root) = failing_bootstrap_app();

    run_until_bootstrap_ready(&mut app);
    app.update();
    app.update();

    assert_eq!(current_state(&app), AppState::MainMenu);
}

/// One simulated frame of real time, so the startup timeout is reached in a
/// handful of frames instead of five wall-clock seconds.
const FRAME: Duration = Duration::from_secs(1);

/// Pump frames until the barrier's timeout has elapsed, checking on the way
/// that nothing but the timeout releases a stalled read.
///
/// Driven by the clock the barrier itself measures rather than by a frame
/// count: the first frame reports a zero delta, so the two disagree.
fn advance_past_timeout(app: &mut App) {
    while app.world().resource::<Time<Real>>().elapsed() < BOOTSTRAP_TIMEOUT {
        assert_eq!(
            app.world().resource::<BootstrapStatus>().localization,
            BootstrapTaskState::Pending,
            "a read that never returns must hold the barrier until the timeout"
        );
        app.update();
    }
}

/// An asset reader whose reads never return.
///
/// A missing or malformed file (TC-047) still *completes* the read. This covers
/// the other shape: a source that never resolves at all, where nothing but the
/// barrier's own timeout can release it.
struct StalledReader;

impl AssetReader for StalledReader {
    async fn read<'a>(&'a self, _path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // Pends forever; the annotation only fixes the `Reader` type that the
        // read would have produced.
        std::future::pending::<Result<Box<dyn Reader>, AssetReaderError>>().await
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // The project ships no `.meta` sidecars; the loader is resolved from
        // the requested asset type instead.
        Err::<Box<dyn Reader>, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }
}

/// An app whose startup catalog reads never return, with real time under test
/// control so the timeout is reached deterministically.
fn stalled_bootstrap_app() -> (App, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("temporary bootstrap root");

    let mut app = App::new();
    // Asset sources must be registered before `AssetPlugin` builds them.
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSourceBuilder::new(|| Box::new(StalledReader)),
    );
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        GameInfrastructurePlugin,
    ));
    // The barrier measures its timeout against `Time<Real>`, so each `update()`
    // advances it by a fixed step instead of by the wall clock.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(FRAME));
    app.insert_resource(BootstrapPaths {
        // Never written, so settings resolve to built-in defaults.
        settings: Some(root.path().join("settings.ron")),
    });
    app.world_mut().resource_mut::<Time<Virtual>>().pause();

    (app, root)
}

// integration-system/application-lifecycle::TC-009
#[test]
fn a_startup_read_that_never_returns_resolves_on_the_timeout() {
    let (mut app, _root) = stalled_bootstrap_app();

    advance_past_timeout(&mut app);

    let status = *app.world().resource::<BootstrapStatus>();
    assert_eq!(status.settings, BootstrapTaskState::Resolved);
    assert_eq!(status.localization, BootstrapTaskState::Resolved);
    assert!(status.is_ready(), "the timeout must release the barrier");
}

// integration-system/application-lifecycle::TC-009
#[test]
fn a_timed_out_bootstrap_keeps_its_diagnostics_and_built_in_defaults() {
    let (mut app, _root) = stalled_bootstrap_app();

    advance_past_timeout(&mut app);

    let diagnostics = app.world().resource::<BootstrapDiagnostics>();
    assert_eq!(
        diagnostics.localization.len(),
        2,
        "each stalled catalog must leave a diagnostic"
    );
    for error in &diagnostics.localization {
        match &error.cause {
            DataErrorCause::Io(reason) => assert!(
                reason.contains("timed out"),
                "a stalled read must be reported as a timeout, got {reason:?}"
            ),
            other => panic!("a stalled read must keep an Io cause, got {other:?}"),
        }
    }

    assert_eq!(
        app.world().resource::<UserSettings>(),
        &UserSettings::default(),
        "settings fall back to complete built-in defaults"
    );
    let localization = app.world().resource::<Localization>();
    assert!(
        localization.catalogs.contains_key(DEFAULT_LOCALE),
        "the built-in English catalog must stand in for the stalled reads"
    );
    assert_eq!(
        localization.text("main_menu.start"),
        "Start",
        "text queries stay safe after a timeout"
    );
}

// integration-system/application-lifecycle::TC-009
#[test]
fn a_timed_out_bootstrap_still_reaches_main_menu() {
    let (mut app, _root) = stalled_bootstrap_app();

    advance_past_timeout(&mut app);
    app.update();
    app.update();

    assert_eq!(
        current_state(&app),
        AppState::MainMenu,
        "a stalled startup read must not strand the app in Boot"
    );
}

/// Guards against the barrier silently passing on fallbacks: with a correct
/// asset root the real catalogs must actually be read.
#[test]
fn a_healthy_bootstrap_reads_the_real_catalogs_without_diagnostics() {
    let mut app = common::controlled_app();
    run_until_bootstrap_ready(&mut app);

    let diagnostics = app.world().resource::<BootstrapDiagnostics>();
    assert!(
        diagnostics.localization.is_empty(),
        "the shipped catalogs must load cleanly: {:?}",
        diagnostics.localization
    );

    let localization = app.world().resource::<client::i18n::Localization>();
    assert!(
        localization.catalogs.contains_key("zh-CN"),
        "the zh-CN catalog must come from assets/, not a built-in fallback"
    );
}
