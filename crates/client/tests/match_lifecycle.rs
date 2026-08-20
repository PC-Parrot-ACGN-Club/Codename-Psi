//! Match instance lifecycle coverage from `integration-system/match-lifecycle.md`.

mod presentation_common;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use client::app_state::{AppState, AppStatePlugin, AppTransitionCause, AppTransitionRequests};
use client::data::{DataResolution, RulesData};
use client::input::LocalInputSampler;
use client::match_flow::{
    AiPlanState, FrozenMatch, MatchFlowPlugin, MatchInstanceId, MatchLifecycleDiagnostics,
    MatchPresentationResources, MatchSelection, RematchIntent, SelectedMode,
};
use client::page::MatchMode;
use client::simulation::{RulesSimulation, SimulationPlugin};
use game_core::{
    board::{Board, Cell, Coord},
    input::{GameAction, PlayerActions, TickInputs},
    match_state::MatchPhase,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        AppStatePlugin,
        MatchFlowPlugin,
        SimulationPlugin,
    ));
    app.init_resource::<LocalInputSampler>();
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app
}

fn current(app: &App) -> AppState {
    *app.world().resource::<State<AppState>>().get()
}

fn submit(app: &mut App, target: AppState, cause: AppTransitionCause) {
    app.world_mut()
        .resource_mut::<AppTransitionRequests>()
        .submit(target, cause);
}

fn commit(app: &mut App) {
    app.update();
    app.update();
}

fn to_character_select(app: &mut App) {
    submit(app, AppState::MainMenu, AppTransitionCause::BootstrapReady);
    commit(app);
    submit(app, AppState::ModeSelect, AppTransitionCause::StartGame);
    commit(app);
    submit(
        app,
        AppState::CharacterSelect,
        AppTransitionCause::ModeConfirmed,
    );
    commit(app);
}

/// An app sitting in `CharacterSelect` with everything the client needs to
/// freeze `seed` on its own, so no test has to hand it a specification.
fn selecting_app(seed: u64) -> App {
    let mut app = app();
    app.insert_resource(RulesData {
        resolution: DataResolution::Loaded(presentation_common::library()),
        excluded_characters: Vec::new(),
    });
    app.insert_resource(MatchSelection {
        rule_profile_id: game_core::config::RuleProfileId("fever-r1".into()),
        root_seed: seed,
        characters: [
            game_core::config::CharacterId("alpha".into()),
            game_core::config::CharacterId("beta".into()),
        ],
        confirmed: true,
    });
    to_character_select(&mut app);
    app
}

fn start_match(app: &mut App, seed: u64, cause: AppTransitionCause) {
    app.insert_resource(FrozenMatch(presentation_common::spec(seed)));
    submit(app, AppState::Match, cause);
    commit(app);
}

fn idle() -> TickInputs {
    TickInputs::new([PlayerActions::EMPTY, PlayerActions::EMPTY]).expect("two slots")
}

fn open_play(state: &mut game_core::MatchState) {
    while !state.phase().is_playing() {
        state.step(&idle()).expect("intro advances");
    }
}

fn stack_spawn_column(state: &mut game_core::MatchState, loser: usize) {
    let geometry = state.spec().board_geometry;
    let mut board = Board::with_geometry(geometry);
    for y in geometry.hidden_rows()..geometry.height() {
        board.set(
            Coord::new(geometry.spawn_column(), y).expect("coord"),
            Cell::Color(y % 2),
        );
    }
    state
        .round_mut()
        .player_mut(loser)
        .expect("slot")
        .set_board(board);
}

fn play_round_lost_by(state: &mut game_core::MatchState, loser: usize) {
    open_play(state);
    stack_spawn_column(state, loser);
    let hard_drop = TickInputs::new([
        PlayerActions::from(GameAction::HardDrop),
        PlayerActions::from(GameAction::HardDrop),
    ])
    .expect("two slots");
    for _ in 0..400 {
        let report = state.step(&hard_drop).expect("tick");
        if report
            .events
            .iter()
            .any(|event| matches!(event, game_core::match_state::MatchEvent::RoundEnded(_)))
        {
            break;
        }
    }
    for _ in 0..=state.spec().round_outro_ticks {
        if matches!(state.phase(), MatchPhase::Completed(_)) {
            break;
        }
        state.step(&idle()).expect("outro advances");
    }
}

// integration-system/match-lifecycle::TC-001
#[test]
fn character_confirmed_creates_a_zero_tick_zero_score_instance_from_frozen_data() {
    let mut app = app();
    to_character_select(&mut app);
    start_match(&mut app, 7, AppTransitionCause::CharacterConfirmed);

    let simulation = app.world().resource::<RulesSimulation>();
    assert_eq!(simulation.0.spec().root_seed, 7);
    assert_eq!(simulation.0.match_tick(), 0);
    assert_eq!(simulation.0.wins(), [0, 0]);
    assert!(app.world().get_resource::<MatchInstanceId>().is_some());
}

// integration-system/match-lifecycle::TC-002
#[test]
fn resume_preserves_instance_identity_tick_checksum_and_wins() {
    let mut app = app();
    to_character_select(&mut app);
    start_match(&mut app, 7, AppTransitionCause::CharacterConfirmed);
    {
        let mut simulation = app.world_mut().resource_mut::<RulesSimulation>();
        play_round_lost_by(&mut simulation.0, 1);
    }
    let before_id = *app.world().resource::<MatchInstanceId>();
    let before = app.world().resource::<RulesSimulation>();
    let before_facts = (before.0.match_tick(), before.0.checksum(), before.0.wins());

    submit(
        &mut app,
        AppState::Paused,
        AppTransitionCause::PauseRequested,
    );
    commit(&mut app);
    submit(
        &mut app,
        AppState::Match,
        AppTransitionCause::ResumeRequested,
    );
    commit(&mut app);

    let after = app.world().resource::<RulesSimulation>();
    assert_eq!(*app.world().resource::<MatchInstanceId>(), before_id);
    assert_eq!(
        (after.0.match_tick(), after.0.checksum(), after.0.wins()),
        before_facts
    );
    assert_eq!(after.0.wins(), [1, 0]);
}

// integration-system/match-lifecycle::TC-003
#[test]
fn restart_rebuilds_with_the_same_spec_seed_and_opening_sequence() {
    let mut app = app();
    to_character_select(&mut app);
    start_match(&mut app, 7, AppTransitionCause::CharacterConfirmed);
    let opening = app.world().resource::<RulesSimulation>().0.view().players[0]
        .next
        .clone();
    let old_id = *app.world().resource::<MatchInstanceId>();
    app.world_mut()
        .resource_mut::<RulesSimulation>()
        .0
        .step(&idle())
        .expect("tick");
    submit(
        &mut app,
        AppState::Paused,
        AppTransitionCause::PauseRequested,
    );
    commit(&mut app);
    submit(
        &mut app,
        AppState::Match,
        AppTransitionCause::RestartRequested,
    );
    commit(&mut app);

    let restarted = app.world().resource::<RulesSimulation>();
    assert_ne!(*app.world().resource::<MatchInstanceId>(), old_id);
    assert_eq!(restarted.0.spec().root_seed, 7);
    assert_eq!(restarted.0.match_tick(), 0);
    assert_eq!(restarted.0.wins(), [0, 0]);
    assert_eq!(restarted.0.view().players[0].next, opening);
}

// integration-system/match-lifecycle::TC-004
#[test]
fn rematch_uses_a_new_frozen_seed_and_resets_the_match() {
    // No spec is injected anywhere in this test: the client has to freeze both
    // the first match and the rematch itself, which is the only way the seed it
    // chooses for the rematch can be observed.
    let mut app = selecting_app(7);
    commit(&mut app);
    assert_eq!(current(&app), AppState::Match);
    let first = app.world().resource::<RulesSimulation>().0.spec().clone();
    let opening = app.world().resource::<RulesSimulation>().0.view().players[0]
        .next
        .clone();
    assert_eq!(first.root_seed, 7);

    submit(
        &mut app,
        AppState::Result,
        AppTransitionCause::MatchCompleted,
    );
    commit(&mut app);
    app.insert_resource(RematchIntent);
    commit(&mut app);
    assert_eq!(current(&app), AppState::Match);

    let rematch = app.world().resource::<RulesSimulation>();
    let second = rematch.0.spec();
    assert_ne!(
        second.root_seed, first.root_seed,
        "a rematch is a new match"
    );
    assert_eq!(rematch.0.match_tick(), 0);
    assert_eq!(rematch.0.wins(), [0, 0]);
    // Same configuration, different match: everything but the seed carries over.
    assert_eq!(second.characters, first.characters);
    assert_eq!(second.profile_id, first.profile_id);
    assert_eq!(second.rule_version, first.rule_version);
    assert_ne!(
        rematch.0.view().players[0].next,
        opening,
        "a new seed produces a different opening sequence"
    );
}

// integration-system/match-lifecycle::TC-010
#[test]
fn a_frozen_spec_never_outlives_the_match_it_was_frozen_for() {
    let mut app = selecting_app(7);
    commit(&mut app);
    assert!(
        app.world().get_resource::<FrozenMatch>().is_none(),
        "the entry that instantiated the spec also consumed it"
    );
    assert_eq!(
        app.world().resource::<RulesSimulation>().0.spec().root_seed,
        7
    );

    // Abandoning to the main menu and playing again must not replay the seed
    // the abandoned match was frozen with.
    submit(
        &mut app,
        AppState::Paused,
        AppTransitionCause::PauseRequested,
    );
    commit(&mut app);
    submit(
        &mut app,
        AppState::MainMenu,
        AppTransitionCause::MatchAbandoned,
    );
    commit(&mut app);
    assert!(app.world().get_resource::<FrozenMatch>().is_none());

    app.world_mut().resource_mut::<MatchSelection>().root_seed = 41;
    submit(
        &mut app,
        AppState::ModeSelect,
        AppTransitionCause::StartGame,
    );
    commit(&mut app);
    submit(
        &mut app,
        AppState::CharacterSelect,
        AppTransitionCause::ModeConfirmed,
    );
    commit(&mut app);
    commit(&mut app);
    assert_eq!(current(&app), AppState::Match);
    assert_eq!(
        app.world().resource::<RulesSimulation>().0.spec().root_seed,
        41,
        "the second match uses its own selection, not the abandoned one"
    );
}

// integration-system/match-lifecycle::TC-005
#[test]
fn leaving_match_for_result_or_main_menu_releases_match_scoped_resources() {
    for (target, cause) in [
        (AppState::Result, AppTransitionCause::MatchCompleted),
        (AppState::MainMenu, AppTransitionCause::MatchAbandoned),
    ] {
        let mut app = app();
        to_character_select(&mut app);
        start_match(&mut app, 7, AppTransitionCause::CharacterConfirmed);
        if cause == AppTransitionCause::MatchAbandoned {
            submit(
                &mut app,
                AppState::Paused,
                AppTransitionCause::PauseRequested,
            );
            commit(&mut app);
        }
        submit(&mut app, target, cause);
        commit(&mut app);
        assert_eq!(current(&app), target);
        assert!(app.world().get_resource::<RulesSimulation>().is_none());
        assert!(app.world().get_resource::<AiPlanState>().is_none());
        assert!(
            app.world()
                .get_resource::<MatchPresentationResources>()
                .is_none()
        );
    }
}

// integration-system/match-lifecycle::TC-006
#[test]
fn pause_and_resume_keep_all_match_scoped_resources() {
    let mut app = app();
    to_character_select(&mut app);
    start_match(&mut app, 7, AppTransitionCause::CharacterConfirmed);
    let id = *app.world().resource::<MatchInstanceId>();
    submit(
        &mut app,
        AppState::Paused,
        AppTransitionCause::PauseRequested,
    );
    commit(&mut app);

    assert_eq!(*app.world().resource::<MatchInstanceId>(), id);
    assert!(app.world().get_resource::<RulesSimulation>().is_some());
    assert!(
        app.world()
            .get_resource::<MatchPresentationResources>()
            .is_some()
    );
}

// integration-system/match-lifecycle::TC-007
#[test]
fn creation_failure_leaves_no_half_initialized_instance_and_can_retry() {
    let mut app = app();
    to_character_select(&mut app);
    submit(
        &mut app,
        AppState::Match,
        AppTransitionCause::CharacterConfirmed,
    );
    commit(&mut app);
    assert!(app.world().get_resource::<RulesSimulation>().is_none());
    assert_eq!(
        app.world().resource::<MatchLifecycleDiagnostics>().0.len(),
        1
    );

    submit(
        &mut app,
        AppState::Result,
        AppTransitionCause::MatchCompleted,
    );
    commit(&mut app);
    start_match(&mut app, 9, AppTransitionCause::RematchRequested);
    assert!(app.world().get_resource::<RulesSimulation>().is_some());
}

// integration-system/match-lifecycle::TC-008
#[test]
fn a_completed_match_requests_result_only_once() {
    let mut app = app();
    to_character_select(&mut app);
    start_match(&mut app, 7, AppTransitionCause::CharacterConfirmed);
    {
        let mut simulation = app.world_mut().resource_mut::<RulesSimulation>();
        play_round_lost_by(&mut simulation.0, 1);
        play_round_lost_by(&mut simulation.0, 1);
    }
    for _ in 0..10 {
        app.world_mut().run_schedule(FixedUpdate);
    }
    assert_eq!(
        app.world()
            .resource::<AppTransitionRequests>()
            .pending
            .len(),
        1
    );
    commit(&mut app);
    assert_eq!(current(&app), AppState::Result);
}

// integration-system/match-lifecycle::TC-009
#[test]
fn single_player_and_local_versus_can_both_complete_a_bo3_to_result() {
    for mode in [
        client::page::MatchMode::SinglePlayer,
        client::page::MatchMode::LocalVersus,
    ] {
        let mut selection = client::page::CharacterSelectPage::new(
            mode,
            vec![
                game_core::config::CharacterId("alpha".into()),
                game_core::config::CharacterId("beta".into()),
            ],
        );
        selection.handle_player(0, client::input::UIAction::Confirm);
        // The second slot is picked by whoever owns it: the one player present
        // when there is no second human, and player 2 otherwise.
        if mode.one_selector() {
            selection.handle_player(0, client::input::UIAction::Confirm);
        } else {
            selection.handle_player(1, client::input::UIAction::Confirm);
        }
        assert!(selection.confirm_enabled());

        let mut app = app();
        to_character_select(&mut app);
        start_match(&mut app, 21, AppTransitionCause::CharacterConfirmed);
        {
            let mut simulation = app.world_mut().resource_mut::<RulesSimulation>();
            play_round_lost_by(&mut simulation.0, 1);
            play_round_lost_by(&mut simulation.0, 1);
            assert_eq!(
                simulation.0.outcome().map(|outcome| outcome.winner),
                Some(0)
            );
            assert_eq!(simulation.0.wins(), [2, 0]);
        }
        app.world_mut().run_schedule(FixedUpdate);
        commit(&mut app);
        assert_eq!(current(&app), AppState::Result);
        submit(
            &mut app,
            AppState::MainMenu,
            AppTransitionCause::ReturnToMainMenu,
        );
        commit(&mut app);
        assert_eq!(current(&app), AppState::MainMenu);
    }
}

// integration-system/match-lifecycle::TC-011
#[test]
fn each_mode_hands_the_ai_exactly_the_slots_no_local_player_owns() {
    for (mode, expected) in [
        (MatchMode::SinglePlayer, vec![1]),
        (MatchMode::LocalVersus, vec![]),
        (MatchMode::AiVersus, vec![0, 1]),
    ] {
        let mut app = app();
        app.insert_resource(SelectedMode(mode));
        to_character_select(&mut app);
        start_match(&mut app, 21, AppTransitionCause::CharacterConfirmed);

        let driven: Vec<usize> = app
            .world()
            .resource::<AiPlanState>()
            .0
            .keys()
            .copied()
            .collect();
        assert_eq!(driven, expected, "{mode:?} drives the wrong slots");
    }
}

// integration-system/match-lifecycle::TC-011
#[test]
fn one_selector_modes_let_a_single_player_pick_both_characters() {
    let roster = vec![
        game_core::config::CharacterId("alpha".into()),
        game_core::config::CharacterId("beta".into()),
    ];

    for mode in [MatchMode::SinglePlayer, MatchMode::AiVersus] {
        assert!(mode.one_selector(), "{mode:?} has no second local player");
        let mut page = client::page::CharacterSelectPage::new(mode, roster.clone());

        // Player 2 owns no slot here, so their input has to be inert.
        page.handle_player(1, client::input::UIAction::Confirm);
        assert!(!page.confirm_enabled(), "{mode:?} took player 2's confirm");

        page.handle_player(0, client::input::UIAction::Confirm);
        // Slot 1 starts one roster entry ahead of slot 0, so the very next
        // confirm already lands on a different character.
        page.handle_player(0, client::input::UIAction::Confirm);
        assert!(page.confirm_enabled(), "{mode:?} left a slot unpicked");
        assert_eq!(
            page.selected()
                .iter()
                .map(|id| id.as_ref().map(|id| id.0.as_str()))
                .collect::<Vec<_>>(),
            vec![Some("alpha"), Some("beta")],
            "{mode:?} did not let one player pick two different characters"
        );
    }
}
