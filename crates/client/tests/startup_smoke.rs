//! Automated startup smoke and the basic main state path.
//!
//! The smoke test runs the same project root plugin the production client uses,
//! on a minimal Bevy runtime with no real window, and returns normally.

mod common;

use bevy::prelude::*;
use client::GameInfrastructurePlugin;
use client::app_state::{AppState, AppTransitionCause};
use client::bootstrap::{BootstrapStatus, BootstrapTaskState};
use client::input::{LocalInputSampler, fixed_pause_inputs};
use common::{
    ALL_STATES, commit, controlled_app, current_state, install_sampler, run_until_bootstrap_ready,
    submit,
};

/// Counts how often each state's gated run phase executed.
#[derive(Debug, Default, Resource)]
struct PhaseRuns(Vec<AppState>);

fn observed_client_app() -> App {
    let mut app = controlled_app();
    app.init_resource::<PhaseRuns>();
    for state in ALL_STATES {
        app.add_systems(
            Update,
            (move |mut runs: ResMut<PhaseRuns>| runs.0.push(state)).run_if(in_state(state)),
        );
    }
    app
}

fn assert_single_current_state(app: &App, expected: AppState) {
    assert_eq!(current_state(app), expected);
    for other in ALL_STATES {
        if other != expected {
            assert_ne!(
                current_state(app),
                other,
                "exactly one top-level state may be current"
            );
        }
    }
}

fn phase_ran(app: &App, state: AppState) -> bool {
    app.world().resource::<PhaseRuns>().0.contains(&state)
}

fn clear_phases(app: &mut App) {
    app.world_mut().resource_mut::<PhaseRuns>().0.clear();
}

/// integration-system/build-and-startup::TC-003
///
/// The platform axis is supplied by the CI job this test runs in; `test.yml`
/// executes it on the Linux runner, the sole target platform for R1 and R2.
#[test]
fn startup_smoke_reuses_the_root_plugin_and_reaches_main_menu() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, GameInfrastructurePlugin));

    assert_eq!(
        current_state(&app),
        AppState::Boot,
        "AppState initializes to Boot"
    );

    // Catalog reads are asynchronous, so pump until the barrier settles rather
    // than assuming a fixed frame count.
    run_until_bootstrap_ready(&mut app);

    let status = *app.world().resource::<BootstrapStatus>();
    assert_eq!(
        status.settings,
        BootstrapTaskState::Resolved,
        "UserSettings completes bootstrap resolution"
    );
    assert_eq!(
        status.localization,
        BootstrapTaskState::Resolved,
        "Localization completes bootstrap resolution"
    );

    app.update();
    app.update();

    assert_eq!(current_state(&app), AppState::MainMenu);
}

// integration-system/application-lifecycle::TC-008
#[test]
fn the_basic_main_path_keeps_one_current_state_at_every_step() {
    let mut app = observed_client_app();
    install_sampler(&mut app, 1);

    assert_single_current_state(&app, AppState::Boot);

    // Boot -> MainMenu is released by the startup barrier itself.
    run_until_bootstrap_ready(&mut app);
    commit(&mut app);
    assert_single_current_state(&app, AppState::MainMenu);
    assert!(phase_ran(&app, AppState::MainMenu));

    clear_phases(&mut app);
    submit(
        &mut app,
        AppState::ModeSelect,
        AppTransitionCause::StartGame,
    );
    commit(&mut app);
    assert_single_current_state(&app, AppState::ModeSelect);
    assert!(phase_ran(&app, AppState::ModeSelect));

    clear_phases(&mut app);
    submit(
        &mut app,
        AppState::CharacterSelect,
        AppTransitionCause::ModeConfirmed,
    );
    commit(&mut app);
    assert_single_current_state(&app, AppState::CharacterSelect);
    assert!(phase_ran(&app, AppState::CharacterSelect));

    clear_phases(&mut app);
    submit(
        &mut app,
        AppState::Match,
        AppTransitionCause::CharacterConfirmed,
    );
    commit(&mut app);
    assert_single_current_state(&app, AppState::Match);
    assert!(phase_ran(&app, AppState::Match));

    // Pause is proposed by the fixed Start button, not by a UI action.
    clear_phases(&mut app);
    app.world_mut()
        .resource_mut::<LocalInputSampler>()
        .press_pause(&fixed_pause_inputs()[0]);
    commit(&mut app);
    assert_single_current_state(&app, AppState::Paused);
    assert!(phase_ran(&app, AppState::Paused));

    clear_phases(&mut app);
    submit(
        &mut app,
        AppState::Match,
        AppTransitionCause::ResumeRequested,
    );
    commit(&mut app);
    assert_single_current_state(&app, AppState::Match);
    assert!(phase_ran(&app, AppState::Match));

    clear_phases(&mut app);
    submit(
        &mut app,
        AppState::Result,
        AppTransitionCause::MatchCompleted,
    );
    commit(&mut app);
    assert_single_current_state(&app, AppState::Result);
    assert!(phase_ran(&app, AppState::Result));

    clear_phases(&mut app);
    submit(
        &mut app,
        AppState::MainMenu,
        AppTransitionCause::ReturnToMainMenu,
    );
    commit(&mut app);
    assert_single_current_state(&app, AppState::MainMenu);
    assert!(phase_ran(&app, AppState::MainMenu));
}
