//! Pure focus and page-action coverage from `component/page-navigation.md`.

use client::app_state::{AppState, AppTransitionCause, SettingsOrigin};
use client::input::UIAction;
use client::page::{
    CharacterSelectPage, FocusItem, FocusRing, MatchMode, PageCommand, PageItem, PageModel,
};
use game_core::config::CharacterId;

fn item(id: PageItem, enabled: bool) -> FocusItem {
    FocusItem::new(id, enabled, (!enabled).then_some("available in R2"))
}

// component/page-navigation::TC-001
#[test]
fn focus_wraps_at_both_ends_and_keeps_one_current_item() {
    let mut ring = FocusRing::new(vec![
        item(PageItem::StartGame, true),
        item(PageItem::Settings, true),
        item(PageItem::Exit, true),
    ]);

    ring.move_focus(UIAction::Up);
    assert_eq!(ring.focused_index(), 2);
    for expected in [0, 1, 2] {
        ring.move_focus(UIAction::Down);
        assert_eq!(ring.focused_index(), expected);
        assert_eq!(ring.items().iter().filter(|item| item.focused).count(), 1);
    }
}

// component/page-navigation::TC-002
#[test]
fn every_direction_is_a_quiet_no_op_on_a_single_item_ring() {
    let mut ring = FocusRing::new(vec![item(PageItem::Back, true)]);
    for action in [
        UIAction::Up,
        UIAction::Down,
        UIAction::Left,
        UIAction::Right,
    ] {
        ring.move_focus(action);
    }
    assert_eq!(ring.focused_index(), 0);
    assert!(ring.diagnostics().is_empty());
}

// component/page-navigation::TC-003
#[test]
fn disabled_items_can_receive_focus_but_cannot_be_confirmed() {
    let mut ring = FocusRing::new(vec![
        item(PageItem::SinglePlayer, true),
        item(PageItem::Lan, false),
    ]);
    ring.move_focus(UIAction::Down);

    assert_eq!(ring.focused().id, PageItem::Lan);
    assert_eq!(
        ring.focused().unavailable_reason.as_deref(),
        Some("available in R2")
    );
    assert_eq!(ring.confirm(), None);
}

// component/page-navigation::TC-004
#[test]
fn enabled_page_items_map_to_the_declared_unique_command() {
    let cases = [
        (
            AppState::MainMenu,
            PageItem::StartGame,
            PageCommand::transition(AppState::ModeSelect, AppTransitionCause::StartGame),
        ),
        (
            AppState::MainMenu,
            PageItem::Settings,
            PageCommand::transition(AppState::Settings, AppTransitionCause::SettingsOpened),
        ),
        (
            AppState::MainMenu,
            PageItem::Exit,
            PageCommand::ExitApplication,
        ),
        (
            AppState::ModeSelect,
            PageItem::SinglePlayer,
            PageCommand::transition(AppState::CharacterSelect, AppTransitionCause::ModeConfirmed),
        ),
        (
            AppState::ModeSelect,
            PageItem::LocalVersus,
            PageCommand::transition(AppState::CharacterSelect, AppTransitionCause::ModeConfirmed),
        ),
        (
            AppState::ModeSelect,
            PageItem::Back,
            PageCommand::transition(AppState::MainMenu, AppTransitionCause::BackRequested),
        ),
        (
            AppState::CharacterSelect,
            PageItem::ConfirmCharacters,
            PageCommand::transition(AppState::Match, AppTransitionCause::CharacterConfirmed),
        ),
        (
            AppState::CharacterSelect,
            PageItem::Back,
            PageCommand::transition(AppState::ModeSelect, AppTransitionCause::BackRequested),
        ),
        (
            AppState::Paused,
            PageItem::Resume,
            PageCommand::transition(AppState::Match, AppTransitionCause::ResumeRequested),
        ),
        (
            AppState::Paused,
            PageItem::Restart,
            PageCommand::transition(AppState::Match, AppTransitionCause::RestartRequested),
        ),
        (
            AppState::Paused,
            PageItem::Settings,
            PageCommand::transition(AppState::Settings, AppTransitionCause::SettingsOpened),
        ),
        (
            AppState::Paused,
            PageItem::ReturnToMainMenu,
            PageCommand::transition(AppState::MainMenu, AppTransitionCause::MatchAbandoned),
        ),
        (
            AppState::Result,
            PageItem::Rematch,
            PageCommand::transition(AppState::Match, AppTransitionCause::RematchRequested),
        ),
        (
            AppState::Result,
            PageItem::ReturnToMainMenu,
            PageCommand::transition(AppState::MainMenu, AppTransitionCause::ReturnToMainMenu),
        ),
    ];

    for (state, target_item, expected) in cases {
        let mut page = PageModel::for_state(state, None).expect("page has a focus ring");
        page.focus_item(target_item).expect("declared item exists");
        assert_eq!(
            page.handle(UIAction::Confirm),
            Some(expected),
            "{state:?}/{target_item:?}"
        );
    }
}

// component/page-navigation::TC-005
#[test]
fn back_is_dropped_without_a_target_and_settings_uses_its_origin() {
    for state in [AppState::MainMenu, AppState::Result] {
        let mut page = PageModel::for_state(state, None).expect("page exists");
        assert_eq!(page.handle(UIAction::Back), None);
    }

    for origin in [AppState::MainMenu, AppState::Paused] {
        let mut page = PageModel::for_state(AppState::Settings, Some(SettingsOrigin(origin)))
            .expect("settings page exists");
        assert_eq!(
            page.handle(UIAction::Back),
            Some(PageCommand::transition(
                origin,
                AppTransitionCause::SettingsClosed
            ))
        );
    }
}

// component/page-navigation::TC-006
#[test]
fn lan_entry_is_focusable_disabled_and_never_starts_a_match() {
    let mut page = PageModel::for_state(AppState::ModeSelect, None).expect("page exists");
    page.focus_item(PageItem::Lan).expect("LAN item exists");
    assert!(!page.focused().enabled);
    assert!(
        page.focused()
            .unavailable_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("R2"))
    );
    assert_eq!(page.handle(UIAction::Confirm), None);
}

fn characters() -> Vec<CharacterId> {
    ["psi-a", "psi-b", "psi-c"]
        .into_iter()
        .map(|id| CharacterId(id.into()))
        .collect()
}

// component/page-navigation::TC-007
#[test]
fn local_character_selection_keeps_player_focus_rings_isolated() {
    let mut page = CharacterSelectPage::new(MatchMode::LocalVersus, characters());
    page.handle_player(0, UIAction::Down);
    assert_eq!(
        [page.focused_index(0), page.focused_index(1)],
        [Some(1), Some(0)]
    );
    page.handle_player(1, UIAction::Down);
    assert_eq!(
        [page.focused_index(0), page.focused_index(1)],
        [Some(1), Some(1)]
    );
}

// component/page-navigation::TC-008
#[test]
fn shared_pages_accept_either_player_while_single_player_ignores_p2() {
    let mut shared = PageModel::for_state(AppState::MainMenu, None).expect("page exists");
    shared.handle_player(0, UIAction::Down);
    shared.handle_player(1, UIAction::Down);
    assert_eq!(shared.focused_index(), 2);

    let mut single = CharacterSelectPage::new(MatchMode::SinglePlayer, characters());
    single.handle_player(1, UIAction::Down);
    assert_eq!(
        [single.focused_index(0), single.focused_index(1)],
        [Some(0), Some(0)]
    );
}

// component/page-navigation::TC-009
#[test]
fn character_confirmation_requires_both_slots_and_allows_duplicates() {
    let mut page = CharacterSelectPage::new(MatchMode::LocalVersus, characters());
    page.handle_player(0, UIAction::Confirm);
    assert!(!page.confirm_enabled());
    assert_eq!(page.confirm_selection(), None);

    page.handle_player(1, UIAction::Confirm);
    assert!(page.confirm_enabled());
    assert_eq!(
        page.selected(),
        [
            Some(&CharacterId("psi-a".into())),
            Some(&CharacterId("psi-a".into()))
        ]
    );
    assert_eq!(
        page.confirm_selection(),
        Some(PageCommand::transition(
            AppState::Match,
            AppTransitionCause::CharacterConfirmed
        ))
    );

    let mut single = CharacterSelectPage::new(MatchMode::SinglePlayer, characters());
    single.handle_player(0, UIAction::Confirm);
    assert!(!single.confirm_enabled());
    single.handle_player(0, UIAction::Down);
    single.handle_player(0, UIAction::Confirm);
    assert!(single.confirm_enabled());
    assert_eq!(single.selected()[0].map(|id| id.0.as_str()), Some("psi-a"));
    assert_eq!(single.selected()[1].map(|id| id.0.as_str()), Some("psi-b"));
}
