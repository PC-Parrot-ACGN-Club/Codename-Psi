//! Page and HUD entity lifecycle, with Bevy's UI stack actually installed.

mod common;
mod presentation_common;

use bevy::prelude::*;
use client::app_state::{AppState, AppTransitionCause};
use common::{advance_to, commit, current_state, submit, ui_app};

/// How many entities carry a component, after letting commands apply.
fn count<C: Component>(app: &mut App) -> usize {
    app.update();
    app.world_mut().query::<&C>().iter(app.world()).count()
}

// integration-system/presentation-runtime::TC-014
#[test]
fn a_page_leaves_no_entity_behind_when_its_state_does() {
    let mut app = ui_app();
    advance_to(&mut app, AppState::MainMenu);
    let main_menu = count::<Node>(&mut app);
    assert!(main_menu > 0, "the main menu builds a page out of nodes");

    submit(
        &mut app,
        AppState::ModeSelect,
        AppTransitionCause::StartGame,
    );
    commit(&mut app);
    let mode_select = count::<Node>(&mut app);
    assert!(mode_select > 0);

    submit(
        &mut app,
        AppState::CharacterSelect,
        AppTransitionCause::ModeConfirmed,
    );
    commit(&mut app);
    assert!(count::<Node>(&mut app) > 0);

    // Walking back out has to arrive at exactly the page that was left, not at
    // that page plus the remains of the ones visited in between.
    submit(
        &mut app,
        AppState::ModeSelect,
        AppTransitionCause::BackRequested,
    );
    commit(&mut app);
    assert_eq!(current_state(&app), AppState::ModeSelect);
    assert_eq!(
        count::<Node>(&mut app),
        mode_select,
        "returning to the mode page must not leave the character page behind"
    );

    submit(
        &mut app,
        AppState::MainMenu,
        AppTransitionCause::BackRequested,
    );
    commit(&mut app);
    assert_eq!(current_state(&app), AppState::MainMenu);
    assert_eq!(
        count::<Node>(&mut app),
        main_menu,
        "the main menu is the same page it was on the way in"
    );
}

// integration-system/presentation-runtime::TC-014
#[test]
fn the_hud_lives_exactly_as_long_as_the_rules_instance() {
    let mut app = ui_app();
    app.insert_resource(client::match_flow::FrozenMatch(presentation_common::spec(
        3,
    )));
    advance_to(&mut app, AppState::Match);

    let with_match = count::<Node>(&mut app);
    assert!(
        app.world()
            .get_resource::<client::simulation::RulesSimulation>()
            .is_some(),
        "the walk into Match has to produce a rules instance for this to mean anything"
    );

    // The pause page sits over a live board, so the HUD stays up under it.
    submit(
        &mut app,
        AppState::Paused,
        AppTransitionCause::PauseRequested,
    );
    commit(&mut app);
    assert_eq!(current_state(&app), AppState::Paused);
    assert!(
        count::<Node>(&mut app) > with_match,
        "pausing adds a page over the HUD instead of replacing it"
    );

    // Leaving for the result page releases the instance, and the HUD with it.
    submit(
        &mut app,
        AppState::Match,
        AppTransitionCause::ResumeRequested,
    );
    commit(&mut app);
    submit(
        &mut app,
        AppState::Result,
        AppTransitionCause::MatchCompleted,
    );
    commit(&mut app);
    assert!(
        app.world()
            .get_resource::<client::simulation::RulesSimulation>()
            .is_none(),
        "the result page outlives the instance by design"
    );
    let on_result = count::<Node>(&mut app);
    assert!(
        on_result * 10 < with_match,
        "the HUD went with its instance rather than lingering: {on_result} nodes \
         on the result page against {with_match} during the match"
    );
    assert_eq!(
        count::<client::hud::BoardCell>(&mut app),
        0,
        "not one board cell outlives the instance it was showing"
    );
}

// integration-system/presentation-runtime::TC-014
#[test]
fn a_clear_leaves_marks_that_expire_on_their_own() {
    use game_core::board::{Board, Cell, Coord};

    let mut app = ui_app();
    app.insert_resource(client::match_flow::FrozenMatch(presentation_common::spec(
        3,
    )));
    advance_to(&mut app, AppState::Match);

    // A board holding one clearing group, so the next lock starts a chain.
    {
        let mut simulation = app
            .world_mut()
            .resource_mut::<client::simulation::RulesSimulation>();
        let geometry = simulation.0.spec().board_geometry;
        let mut board = Board::with_geometry(geometry);
        for x in 0..4 {
            board.set(
                Coord::new(x, geometry.height() - 1).expect("bottom row"),
                Cell::Color(0),
            );
        }
        simulation
            .0
            .round_mut()
            .player_mut(0)
            .expect("slot exists")
            .set_board(board);
    }

    // Hard drop until the clear preview opens and the marks are laid down.
    let mut marks = 0;
    for _ in 0..600 {
        common::press(&mut app, 0, game_core::input::GameAction::HardDrop);
        common::run_fixed_tick(&mut app);
        marks = count::<client::effects::ClearMark>(&mut app);
        if marks > 0 {
            break;
        }
    }
    assert!(marks > 0, "a clear left no mark at all");

    // Nothing else has to clean them up: they go when their own life is over.
    for _ in 0..120 {
        common::run_fixed_tick(&mut app);
    }
    assert_eq!(
        count::<client::effects::ClearMark>(&mut app),
        0,
        "marks outlived their own life instead of expiring"
    );
}
