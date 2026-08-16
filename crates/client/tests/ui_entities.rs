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

    // This batch is what has to disappear. Counting every mark on screen would
    // instead measure the chain's pace, since later links keep laying down new
    // ones while the match plays on.
    let batch: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<client::effects::ClearMark>>()
        .iter(app.world())
        .collect();

    // Nothing else has to clean them up: they go when their own life is over.
    for _ in 0..120 {
        common::run_fixed_tick(&mut app);
    }
    app.update();
    let survivors = batch
        .iter()
        .filter(|entity| {
            app.world()
                .get::<client::effects::ClearMark>(**entity)
                .is_some()
        })
        .count();
    assert_eq!(
        survivors, 0,
        "marks outlived their own life instead of expiring"
    );
}

// integration-system/presentation-runtime::TC-016
#[test]
fn both_portraits_are_drawn_from_the_catalogs_own_colours_and_badges() {
    let mut app = ui_app();
    app.insert_resource(client::match_flow::FrozenMatch(presentation_common::spec(
        3,
    )));
    advance_to(&mut app, AppState::Match);

    // The catalog is degradable data, so it settles on its own schedule --
    // after this match already started, which is the point: the substitute the
    // portraits resolved to in the meantime has to be replaced once the real
    // catalog lands, not held for the rest of the match.
    for _ in 0..2000 {
        app.update();
        if app
            .world()
            .get_resource::<client::data::CharacterPresentationData>()
            .is_some()
        {
            break;
        }
    }
    let catalog = app
        .world()
        .resource::<client::data::CharacterPresentationData>()
        .0
        .loaded()
        .expect("the repository's presentation data loads")
        .clone();
    app.update();

    let mut badges: Vec<String> = app
        .world_mut()
        .query::<(&Text, &TextColor)>()
        .iter(app.world())
        .map(|(text, _)| text.0.clone())
        .collect();
    badges.sort();
    for id in ["psi-a", "psi-b"] {
        let entry = catalog
            .get(&game_core::config::CharacterId(id.into()))
            .expect("the character is in the catalog");
        assert!(
            badges.contains(&entry.badge.glyph),
            "{id}'s badge {:?} is not on screen: {badges:?}",
            entry.badge.glyph
        );
    }

    // The portrait circles carry the characters' own border colours, which is
    // what tells a substitute apart from the real thing.
    let expected: Vec<Color> = ["psi-a", "psi-b"]
        .into_iter()
        .map(|id| {
            let color = catalog
                .get(&game_core::config::CharacterId(id.into()))
                .expect("the character is in the catalog")
                .primary_color;
            Color::srgb_u8(color.r, color.g, color.b)
        })
        .collect();
    let borders: Vec<BorderColor> = app
        .world_mut()
        .query::<&BorderColor>()
        .iter(app.world())
        .copied()
        .collect();
    for color in expected {
        assert!(
            borders.contains(&BorderColor::all(color)),
            "no portrait is drawn with {color:?}"
        );
    }
}

// integration-system/presentation-runtime::TC-015
#[test]
fn the_portrait_name_is_the_localized_roster_name_not_the_drop_set_id() {
    let mut app = ui_app();
    app.insert_resource(client::match_flow::FrozenMatch(presentation_common::spec(
        5,
    )));
    advance_to(&mut app, AppState::Match);

    // The roster arrives with the rest of the runtime data, so the name settles
    // on the data's schedule rather than on the first frame in `Match`.
    for _ in 0..2000 {
        app.update();
        if app
            .world()
            .get_resource::<client::data::RulesData>()
            .is_some_and(|data| data.rules().is_some())
        {
            break;
        }
    }
    app.update();

    let localization = app.world().resource::<client::i18n::Localization>();
    let expected: Vec<String> = ["character.psi_a.name", "character.psi_b.name"]
        .into_iter()
        .map(|key| localization.text(key))
        .collect();
    // A key that resolved to itself would make the assertions below vacuous.
    for (key, name) in ["character.psi_a.name", "character.psi_b.name"]
        .into_iter()
        .zip(&expected)
    {
        assert_ne!(name, key, "{key} has no entry in the active catalog");
    }

    let shown: Vec<String> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect();

    for name in &expected {
        assert!(
            shown.contains(name),
            "the roster name {name:?} is not on screen: {shown:?}"
        );
    }
    // `drop_set_id` is what used to be drawn here; it must not be any more.
    for id in ["psi-a", "psi-b"] {
        assert!(
            !shown.iter().any(|text| text == id),
            "{id:?} is still drawn as a name: {shown:?}"
        );
    }
}

/// Every string currently drawn on screen.
fn screen_text(app: &mut App) -> Vec<String> {
    app.update();
    app.world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect()
}

/// Drive one menu action into the page on screen, as a player would.
fn act(app: &mut App, action: client::input::UIAction) {
    app.world_mut()
        .write_message(client::input::UIActionEvent { player: 0, action });
    app.update();
}

// integration-system/presentation-runtime::TC-014
#[test]
fn walking_into_the_binding_tree_replaces_the_rows_on_screen() {
    use client::input::UIAction;
    use client::page::{PageItem, SettingsPage};
    use client::ui::ActivePage;

    let mut app = ui_app();
    advance_to(&mut app, AppState::MainMenu);
    submit(
        &mut app,
        AppState::Settings,
        AppTransitionCause::SettingsOpened,
    );
    commit(&mut app);

    // The root level draws the general settings and the way into the tree, and
    // no binding row at all: those need a player and a device first.
    let root = screen_text(&mut app);
    assert!(root.iter().any(|text| text == "Language"));
    assert!(root.iter().any(|text| text == "Key Bindings"));
    assert!(
        !root.iter().any(|text| text == "Soft Drop / Confirm"),
        "the root level must not list binding rows, got {root:?}"
    );

    app.world_mut()
        .resource_mut::<ActivePage>()
        .0
        .focus_item(PageItem::InputBindings)
        .expect("the settings page offers the binding tree");
    act(&mut app, UIAction::Confirm);
    assert_eq!(
        app.world().resource::<ActivePage>().0.settings_page(),
        SettingsPage::Players
    );

    // The rows really are replaced, not added to: the general settings are gone
    // from the screen, and the two players have taken their place.
    let players = screen_text(&mut app);
    assert!(
        !players.iter().any(|text| text == "Language"),
        "descending must clear the level above it, got {players:?}"
    );
    assert!(players.iter().any(|text| text == "P1"));
    assert!(players.iter().any(|text| text == "P2"));

    act(&mut app, UIAction::Confirm);
    let devices = screen_text(&mut app);
    assert!(devices.iter().any(|text| text == "Keyboard"));
    // No pad is connected in this harness, so the pad row says why it cannot be
    // opened -- and keeps saying it, which is the whole point of the reason
    // travelling with the row.
    assert!(
        devices
            .iter()
            .any(|text| text.starts_with("Gamepad") && text.contains("no gamepad connected")),
        "the pad row has to say why it is unavailable, got {devices:?}"
    );

    // The keyboard level names its four actions without repeating whose they
    // are or what they are on: the two levels above already said so.
    act(&mut app, UIAction::Confirm);
    let bindings = screen_text(&mut app);
    for action in [
        "Soft Drop",
        "Hard Drop",
        "Rotate CW / Back",
        "Rotate CCW / Confirm",
    ] {
        assert!(
            bindings.iter().any(|text| text == action),
            "the keyboard level must list {action}, got {bindings:?}"
        );
    }
    assert!(
        bindings.iter().any(|text| text == "S"),
        "each row shows the key it is bound to, got {bindings:?}"
    );

    // Backing out arrives at the level entered from, still fully drawn.
    act(&mut app, UIAction::Back);
    act(&mut app, UIAction::Back);
    act(&mut app, UIAction::Back);
    assert_eq!(
        app.world().resource::<ActivePage>().0.settings_page(),
        SettingsPage::Root
    );
    let returned = screen_text(&mut app);
    assert!(returned.iter().any(|text| text == "Language"));
    assert!(
        !returned.iter().any(|text| text == "P1"),
        "the tree must not be left behind under the root level, got {returned:?}"
    );
    assert_eq!(current_state(&app), AppState::Settings);
}

// integration-system/presentation-runtime::TC-014
#[test]
fn a_language_change_keeps_the_reason_a_row_is_unavailable() {
    use client::input::UIAction;
    use client::page::PageItem;
    use client::settings::UserSettings;
    use client::ui::ActivePage;

    let mut app = ui_app();
    advance_to(&mut app, AppState::MainMenu);
    submit(
        &mut app,
        AppState::Settings,
        AppTransitionCause::SettingsOpened,
    );
    commit(&mut app);

    app.world_mut()
        .resource_mut::<ActivePage>()
        .0
        .focus_item(PageItem::InputBindings)
        .expect("the settings page offers the binding tree");
    act(&mut app, UIAction::Confirm);
    act(&mut app, UIAction::Confirm);

    let unavailable = |app: &mut App| {
        screen_text(app)
            .into_iter()
            .find(|text| text.starts_with("Gamepad") || text.starts_with("手柄"))
            .expect("the device level lists the pad")
    };
    assert!(unavailable(&mut app).contains("no gamepad connected"));

    // Editing an unrelated setting once rewrote every row from a model that
    // knew nothing about devices, which silently dropped this line and left a
    // row that was disabled for no stated reason until a pad was plugged in.
    app.world_mut().resource_mut::<UserSettings>().language = "zh-CN".into();
    app.update();
    let translated = unavailable(&mut app);
    assert!(
        translated.contains("未连接手柄"),
        "the reason has to survive a language change, got {translated:?}"
    );
}
