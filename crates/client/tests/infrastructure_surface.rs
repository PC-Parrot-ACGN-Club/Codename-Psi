use bevy::prelude::*;
use client::GameInfrastructurePlugin;
use client::app_state::{AppState, AppTransitionCause, AppTransitionRequests, is_valid_transition};
use client::bootstrap::{BootstrapStatus, BootstrapTaskState};
use client::data::{DataCategory, DataErrorCause, DataLoadError, DataResolution};
use client::i18n::{Localization, builtin_english_catalog};
use client::input::{FixedDirection, LocalInputSampler, PhysicalInput, UIAction};
use client::settings::{PlayerInputBindings, SettingsStore, UserSettings};
use client::simulation::{FIXED_HZ, FixedGameSet, SimulationProbe};
use game_core::input::{GameAction, PlayerActions, TickInputs};

#[test]
fn component_surfaces_are_available_to_external_tests() {
    let settings = UserSettings::default();
    let bindings = PlayerInputBindings::default();
    let mut sampler = LocalInputSampler::new(vec![bindings]);
    sampler.press(0, PhysicalInput::keyboard("KeyS"));
    sampler.press_fixed_direction(0, PhysicalInput::gamepad("DPadLeft"), FixedDirection::Left);
    let sampled = sampler.sample_fixed();

    let localization = Localization::new("en", [builtin_english_catalog()]);
    let resolution = DataResolution::Fallback {
        value: localization.text("main_menu.start"),
        error: DataLoadError {
            path: "assets/i18n/en.json".into(),
            category: DataCategory::Localization,
            cause: DataErrorCause::Io("fixture".into()),
        },
    };
    let tick_inputs = TickInputs::new(sampled).expect("one local participant fits");

    assert_eq!(settings.language, "en");
    assert!(tick_inputs.player(0).is_some());
    assert_eq!(resolution.value(), "Start");
    assert!(is_valid_transition(AppState::Boot, AppState::MainMenu));

    let _ui_action = UIAction::Confirm;
    let _rules_action = PlayerActions::from(GameAction::HardDrop);
    let _store = SettingsStore::new("settings.ron");
    let _fixed_set = FixedGameSet::Input;
    assert_eq!(FIXED_HZ, 60.0);
}

#[test]
fn root_plugin_is_available_to_a_minimal_bevy_app() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, GameInfrastructurePlugin));

    assert_eq!(
        app.world().resource::<State<AppState>>().get(),
        &AppState::Boot
    );
    assert!(app.world().contains_resource::<UserSettings>());
    assert!(app.world().contains_resource::<Localization>());
    assert!(
        !app.world().contains_resource::<SimulationProbe>(),
        "the probe is test instrumentation and must stay out of the production assembly"
    );

    app.world_mut().resource_mut::<BootstrapStatus>().settings = BootstrapTaskState::Resolved;
    app.world_mut()
        .resource_mut::<BootstrapStatus>()
        .localization = BootstrapTaskState::Resolved;
    app.world_mut()
        .resource_mut::<AppTransitionRequests>()
        .submit(AppState::MainMenu, AppTransitionCause::BootstrapReady);

    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<State<AppState>>().get(),
        &AppState::MainMenu
    );
}
