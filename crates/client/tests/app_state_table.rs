//! The pure top-level state table: which edges are valid and which are not.

use client::app_state::{AppState, is_valid_transition};

/// component/application-state-table::TC-001 — one case per basic edge.
macro_rules! valid_edge_cases {
    ($($name:ident => ($from:expr, $to:expr)),+ $(,)?) => {
        $(
            // component/application-state-table::TC-001
            #[test]
            fn $name() {
                assert!(
                    is_valid_transition($from, $to),
                    "{:?} -> {:?} is a basic edge of the state table",
                    $from,
                    $to
                );
            }
        )+
    };
}

valid_edge_cases! {
    boot_to_main_menu_is_valid => (AppState::Boot, AppState::MainMenu),
    main_menu_to_mode_select_is_valid => (AppState::MainMenu, AppState::ModeSelect),
    main_menu_to_settings_is_valid => (AppState::MainMenu, AppState::Settings),
    mode_select_to_character_select_is_valid => (AppState::ModeSelect, AppState::CharacterSelect),
    mode_select_to_main_menu_is_valid => (AppState::ModeSelect, AppState::MainMenu),
    character_select_to_match_is_valid => (AppState::CharacterSelect, AppState::Match),
    character_select_to_mode_select_is_valid => (AppState::CharacterSelect, AppState::ModeSelect),
    settings_to_main_menu_is_valid => (AppState::Settings, AppState::MainMenu),
    settings_to_paused_is_valid => (AppState::Settings, AppState::Paused),
    match_to_paused_is_valid => (AppState::Match, AppState::Paused),
    paused_to_match_is_valid => (AppState::Paused, AppState::Match),
    paused_to_settings_is_valid => (AppState::Paused, AppState::Settings),
    paused_to_main_menu_is_valid => (AppState::Paused, AppState::MainMenu),
    match_to_result_is_valid => (AppState::Match, AppState::Result),
    result_to_match_is_valid => (AppState::Result, AppState::Match),
    result_to_main_menu_is_valid => (AppState::Result, AppState::MainMenu),
}

/// component/application-state-table::TC-002 — one out-of-table target per source state.
macro_rules! invalid_edge_cases {
    ($($name:ident => ($from:expr, $to:expr)),+ $(,)?) => {
        $(
            // component/application-state-table::TC-002
            #[test]
            fn $name() {
                assert!(
                    !is_valid_transition($from, $to),
                    "{:?} -> {:?} is not listed for that source state",
                    $from,
                    $to
                );
            }
        )+
    };
}

invalid_edge_cases! {
    boot_to_match_is_invalid => (AppState::Boot, AppState::Match),
    main_menu_to_match_is_invalid => (AppState::MainMenu, AppState::Match),
    mode_select_to_result_is_invalid => (AppState::ModeSelect, AppState::Result),
    character_select_to_paused_is_invalid => (AppState::CharacterSelect, AppState::Paused),
    match_to_main_menu_is_invalid => (AppState::Match, AppState::MainMenu),
    paused_to_result_is_invalid => (AppState::Paused, AppState::Result),
    settings_to_match_is_invalid => (AppState::Settings, AppState::Match),
    result_to_paused_is_invalid => (AppState::Result, AppState::Paused),
}
