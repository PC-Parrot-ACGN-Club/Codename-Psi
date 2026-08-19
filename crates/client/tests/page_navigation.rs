//! Pure focus and page-action coverage from `component/page-navigation.md`.

use client::app_state::{AppState, AppTransitionCause, SettingsOrigin};
use client::input::UIAction;
use client::page::{
    CharacterSelectPage, FocusItem, FocusRing, MatchMode, PageCommand, PageItem, PageModel,
};
use game_core::config::CharacterId;

fn item(id: PageItem, enabled: bool) -> FocusItem {
    FocusItem::new(
        id,
        enabled,
        (!enabled).then_some("mode_select.lan_unavailable"),
    )
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
        Some("mode_select.lan_unavailable")
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

    // Settings is the one page whose back cause depends on where it was opened,
    // so it cannot join the table above. Confirming the item still has to leave
    // the page on its own: a player who never learns the back *input* would
    // otherwise be stranded in settings.
    for origin in [AppState::MainMenu, AppState::Paused] {
        let mut page = PageModel::for_state(AppState::Settings, Some(SettingsOrigin(origin)))
            .expect("settings page exists");
        page.focus_item(PageItem::Back).expect("back item exists");
        assert_eq!(
            page.handle(UIAction::Confirm),
            Some(PageCommand::transition(
                origin,
                AppTransitionCause::SettingsClosed
            )),
            "settings opened from {origin:?}"
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
    // The reason is a localization key, so the line the player reads is in
    // their language rather than in whatever the model was written in.
    let reason = page
        .focused()
        .unavailable_reason
        .clone()
        .expect("a disabled item says why");
    assert_eq!(reason, "mode_select.lan_unavailable");
    for source in [
        include_str!("../../../assets/i18n/en.json"),
        include_str!("../../../assets/i18n/zh-CN.json"),
    ] {
        let catalog = client::i18n::parse_catalog(source).expect("the shipped catalog parses");
        assert!(
            catalog.messages.contains_key(&reason),
            "every shipped language names the reason"
        );
    }
    assert_eq!(page.handle(UIAction::Confirm), None);
}

/// A settings page opened from the main menu, with focus on a given item.
fn settings_page_focused_on(id: PageItem) -> PageModel {
    let mut page =
        PageModel::for_state(AppState::Settings, Some(SettingsOrigin(AppState::MainMenu)))
            .expect("settings page exists");
    page.focus_item(id).expect("the page lists that item");
    page
}

// component/page-navigation::TC-013
#[test]
fn the_pad_row_follows_whether_a_pad_is_connected() {
    use client::page::SettingsPage;
    use client::settings::DeviceCategory;

    let device_row = |device| PageItem::DeviceBindings { player: 0, device };
    let mut page = settings_page_focused_on(PageItem::InputBindings);
    page.handle(UIAction::Confirm);
    page.focus_item(PageItem::PlayerBindings { player: 0 })
        .expect("the tree lists P1");
    page.handle(UIAction::Confirm);
    assert_eq!(page.settings_page(), SettingsPage::Devices { player: 0 });

    // A model that has not been told about devices assumes none: a row that is
    // wrongly enabled opens a capture that can never complete.
    let pad_row = |page: &PageModel| {
        page.items()
            .iter()
            .find(|item| item.id == device_row(DeviceCategory::Gamepad))
            .cloned()
            .expect("the device level lists the pad")
    };
    assert!(!pad_row(&page).enabled);
    let reason = pad_row(&page)
        .unavailable_reason
        .expect("a disabled row says why");
    assert_eq!(reason, "settings.no_gamepad");
    for source in [
        include_str!("../../../assets/i18n/en.json"),
        include_str!("../../../assets/i18n/zh-CN.json"),
    ] {
        let catalog = client::i18n::parse_catalog(source).expect("the shipped catalog parses");
        assert!(
            catalog.messages.contains_key(&reason),
            "every shipped language names the reason"
        );
    }

    // The list does not shrink and the keyboard row is untouched: one missing
    // device does not take the whole binding surface with it.
    assert_eq!(page.items().len(), 3, "the device level still lists both");
    assert!(
        page.items()
            .iter()
            .find(|item| item.id == device_row(DeviceCategory::Keyboard))
            .is_some_and(|item| item.enabled)
    );

    // A disabled row cannot be walked into.
    page.focus_item(device_row(DeviceCategory::Gamepad))
        .expect("a disabled row still takes focus");
    assert_eq!(page.handle(UIAction::Confirm), None);
    assert_eq!(
        page.settings_page(),
        SettingsPage::Devices { player: 0 },
        "confirming a disabled row must not descend"
    );

    // Plugging a pad in restores it without leaving the page.
    assert!(page.set_gamepad_available(true));
    assert!(pad_row(&page).enabled && pad_row(&page).unavailable_reason.is_none());
    assert!(
        !page.set_gamepad_available(true),
        "an unchanged availability must not ask the page to redraw"
    );

    // Walking away and back rebuilds the level, and the rebuilt one has to
    // still know about the missing pad -- rebuilding without it is how the row
    // once lost the line saying why it was unavailable.
    assert!(page.set_gamepad_available(false));
    page.handle(UIAction::Back);
    page.focus_item(PageItem::PlayerBindings { player: 0 })
        .expect("the tree lists P1");
    page.handle(UIAction::Confirm);
    assert!(!pad_row(&page).enabled);
    assert_eq!(
        pad_row(&page).unavailable_reason.as_deref(),
        Some("settings.no_gamepad"),
        "a rebuilt level still says why the row is unavailable"
    );
}

// component/page-navigation::TC-010
#[test]
fn the_binding_tree_descends_by_player_then_device_and_backs_out_the_way_it_came() {
    use client::page::SettingsPage;
    use client::settings::DeviceCategory;
    use game_core::input::GameAction;

    let ids =
        |page: &PageModel| -> Vec<PageItem> { page.items().iter().map(|item| item.id).collect() };

    // The root level is the general settings, the way into the tree, and back.
    let mut page = settings_page_focused_on(PageItem::InputBindings);
    assert_eq!(page.settings_page(), SettingsPage::Root);
    let root = ids(&page);
    assert!(
        root[..root.len() - 2].iter().all(|id| id.is_setting()),
        "the general settings come first"
    );
    assert_eq!(root[root.len() - 2], PageItem::InputBindings);
    assert_eq!(*root.last().expect("a level ends in back"), PageItem::Back);
    assert!(
        !root.iter().any(|id| matches!(id, PageItem::Rebind { .. })),
        "no binding row is listed before a player and a device were chosen"
    );

    // Confirming descends one level at a time: player, then device, then the
    // four configurable actions.
    page.handle(UIAction::Confirm);
    assert_eq!(page.settings_page(), SettingsPage::Players);
    assert_eq!(
        ids(&page),
        vec![
            PageItem::PlayerBindings { player: 0 },
            PageItem::PlayerBindings { player: 1 },
            PageItem::Back,
        ]
    );

    page.focus_item(PageItem::PlayerBindings { player: 1 })
        .expect("the tree lists P2");
    page.handle(UIAction::Confirm);
    assert_eq!(page.settings_page(), SettingsPage::Devices { player: 1 });
    assert_eq!(
        ids(&page),
        vec![
            PageItem::DeviceBindings {
                player: 1,
                device: DeviceCategory::Keyboard,
            },
            PageItem::DeviceBindings {
                player: 1,
                device: DeviceCategory::Gamepad,
            },
            PageItem::Back,
        ]
    );

    page.handle(UIAction::Confirm);
    assert_eq!(
        page.settings_page(),
        SettingsPage::Bindings {
            player: 1,
            device: DeviceCategory::Keyboard,
        }
    );
    let bindings = ids(&page);
    assert_eq!(
        bindings[..bindings.len() - 1],
        GameAction::CONFIGURABLE.map(|action| PageItem::Rebind {
            player: 1,
            action,
            device: DeviceCategory::Keyboard,
        })
    );
    assert_eq!(
        *bindings.last().expect("a level ends in back"),
        PageItem::Back
    );

    // Back climbs one level and lands on the row that was entered from, so the
    // way out is the way in, reversed.
    assert_eq!(page.handle(UIAction::Back), None);
    assert_eq!(page.settings_page(), SettingsPage::Devices { player: 1 });
    assert_eq!(
        page.focused().id,
        PageItem::DeviceBindings {
            player: 1,
            device: DeviceCategory::Keyboard,
        }
    );

    assert_eq!(page.handle(UIAction::Back), None);
    assert_eq!(page.settings_page(), SettingsPage::Players);
    assert_eq!(page.focused().id, PageItem::PlayerBindings { player: 1 });

    assert_eq!(page.handle(UIAction::Back), None);
    assert_eq!(page.settings_page(), SettingsPage::Root);
    assert_eq!(page.focused().id, PageItem::InputBindings);

    // Only the root's back leaves the page.
    assert_eq!(
        page.handle(UIAction::Back),
        Some(PageCommand::transition(
            AppState::MainMenu,
            AppTransitionCause::SettingsClosed,
        ))
    );
}

// component/page-navigation::TC-014
#[test]
fn confirming_a_sub_level_back_row_climbs_instead_of_leaving_the_page() {
    use client::page::SettingsPage;

    let mut page = settings_page_focused_on(PageItem::InputBindings);
    page.handle(UIAction::Confirm);
    page.focus_item(PageItem::Back)
        .expect("a level ends in back");

    // The row and the input have to agree: a player who reaches `Back` by
    // walking the ring must not be thrown out of the settings page entirely.
    assert_eq!(page.handle(UIAction::Confirm), None);
    assert_eq!(page.settings_page(), SettingsPage::Root);
    assert_eq!(page.focused().id, PageItem::InputBindings);
}

// component/page-navigation::TC-011
#[test]
fn the_corner_legend_names_each_player_s_current_confirm_and_back_keys() {
    use client::i18n::{Localization, parse_catalog};
    use client::input::{GamepadSlots, PhysicalInput};
    use client::settings::UserSettings;
    use client::ui::{key_legend_text, rebind_label};
    use game_core::input::GameAction;

    // The shipped catalog rather than a fixture, so a legend that asks for a
    // key the catalog does not carry fails here instead of on screen.
    const ASSET_EN: &str = include_str!("../../../assets/i18n/en.json");

    let mut settings = UserSettings::default();
    let localization = Localization::new("en", [parse_catalog(ASSET_EN).expect("catalog parses")]);
    // No pad is bound to either slot, so both legends describe the keyboard.
    let slots = GamepadSlots::default();

    // The defaults are the ones the player is told about without opening the
    // settings page at all: J confirms, K goes back.
    let p1 = key_legend_text(0, &settings, &slots, &localization);
    assert!(
        p1.contains("J") && p1.contains("K"),
        "P1's legend must name its default rotation keys, got {p1:?}"
    );
    assert!(p1.starts_with("P1"), "the left corner belongs to P1");

    // P2's defaults differ, so the two corners must not read the same.
    let p2 = key_legend_text(1, &settings, &slots, &localization);
    assert!(p2.starts_with("P2"), "the right corner belongs to P2");
    assert_ne!(p1, p2);

    // Rebinding the rotation moves the legend with it -- that is the whole
    // reason the legend exists rather than a printed constant.
    settings.players[0].bindings.insert(
        GameAction::RotateCounterClockwise,
        vec![PhysicalInput::keyboard("KeyP")],
    );
    let rebound = key_legend_text(0, &settings, &slots, &localization);
    assert!(
        rebound.contains("P") && !rebound.contains("J"),
        "the legend has to follow the binding, got {rebound:?}"
    );

    // The settings row for that binding names both of the things the key does.
    let label = rebind_label(&localization, GameAction::RotateCounterClockwise);
    assert!(
        label.contains("Rotate CCW") && label.contains("Confirm"),
        "the rotation row must name its menu meaning too, got {label:?}"
    );
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
        [Some(0), Some(1)]
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
    // Slot 1 starts one roster entry ahead of slot 0, so the very next
    // confirm already lands on a different character.
    single.handle_player(0, UIAction::Confirm);
    assert!(single.confirm_enabled());
    assert_eq!(single.selected()[0].map(|id| id.0.as_str()), Some("psi-a"));
    assert_eq!(single.selected()[1].map(|id| id.0.as_str()), Some("psi-b"));
}

// component/page-navigation::TC-012
#[test]
fn every_setting_value_is_named_in_the_players_own_language() {
    use client::i18n::{Localization, parse_catalog};
    use client::input::GamepadSlots;
    use client::settings::{AnimationIntensity, UserSettings, WindowModeSetting};
    use client::ui::{language_name, setting_value};

    const ASSET_EN: &str = include_str!("../../../assets/i18n/en.json");
    const ASSET_ZH: &str = include_str!("../../../assets/i18n/zh-CN.json");
    let catalogs = || {
        [
            parse_catalog(ASSET_EN).expect("catalog parses"),
            parse_catalog(ASSET_ZH).expect("catalog parses"),
        ]
    };
    let en = Localization::new("en", catalogs());
    let zh = Localization::new("zh-CN", catalogs());
    let slots = GamepadSlots::default();

    let mut settings = UserSettings {
        window_mode: WindowModeSetting::BorderlessFullscreen,
        animation_intensity: AnimationIntensity::Reduced,
        vibration: true,
        color_assist: false,
        ..UserSettings::default()
    };

    // The values a player picks from a fixed set differ between languages...
    for item in [
        PageItem::WindowMode,
        PageItem::AnimationIntensity,
        PageItem::Vibration,
        PageItem::ColorAssist,
    ] {
        let english = setting_value(item, &settings, &slots, &en);
        let chinese = setting_value(item, &settings, &slots, &zh);
        assert!(
            !english.is_empty() && !chinese.is_empty(),
            "{item:?} is blank"
        );
        assert_ne!(
            english, chinese,
            "{item:?} shows the same text in both languages, so it is not localized"
        );
    }

    // ...while a number is a number.
    settings.master_volume = 0.5;
    assert_eq!(
        setting_value(PageItem::MasterVolume, &settings, &slots, &en),
        "50%"
    );
    assert_eq!(
        setting_value(PageItem::MasterVolume, &settings, &slots, &zh),
        "50%"
    );

    // A language names itself the same way in every catalog, so a player who
    // cannot read the current one can still find their own.
    for locale in ["en", "zh-CN"] {
        assert_eq!(language_name(&en, locale), language_name(&zh, locale));
    }
    assert_eq!(language_name(&en, "en"), "English");
    assert_eq!(language_name(&en, "zh-CN"), "简体中文");
    assert_eq!(
        language_name(&en, "xx-YY"),
        "xx-YY",
        "a locale no catalog names falls back to its code rather than to a key"
    );

    // The language row shows the autonym, not the raw code.
    settings.language = "zh-CN".into();
    assert_eq!(
        setting_value(PageItem::Language, &settings, &slots, &en),
        "简体中文"
    );
}

// component/page-navigation::TC-012
#[test]
fn page_text_is_rebuilt_from_the_current_language() {
    use client::i18n::{Localization, parse_catalog};
    use client::ui::item_label;

    const ASSET_EN: &str = include_str!("../../../assets/i18n/en.json");
    const ASSET_ZH: &str = include_str!("../../../assets/i18n/zh-CN.json");
    let mut localization = Localization::new(
        "en",
        [
            parse_catalog(ASSET_EN).expect("catalog parses"),
            parse_catalog(ASSET_ZH).expect("catalog parses"),
        ],
    );

    let page = PageModel::for_state(AppState::ModeSelect, None).expect("page exists");
    let english: Vec<String> = page
        .items()
        .iter()
        .map(|item| item_label(&localization, item))
        .collect();

    assert!(localization.set_locale("zh-CN"), "the locale is available");
    let chinese: Vec<String> = page
        .items()
        .iter()
        .map(|item| item_label(&localization, item))
        .collect();

    assert_eq!(english.len(), chinese.len());
    for (english, chinese) in english.iter().zip(&chinese) {
        assert_ne!(
            english, chinese,
            "the same page model produced the same line in two languages"
        );
    }
    // The reason a disabled item gives is part of the line, so it follows too.
    let lan = page
        .items()
        .iter()
        .find(|item| item.id == PageItem::Lan)
        .expect("the mode page lists LAN");
    let line = item_label(&localization, lan);
    assert!(
        line.contains('(') && !line.contains("mode_select."),
        "the unavailable reason is shown localized, not as its key: {line:?}"
    );
}
