//! Bevy UI for the client's pages: the visual half of [`crate::page`].
//!
//! [`crate::page`] owns what a page *is* -- its items, focus and commands --
//! and stays free of Bevy so it can be tested as a pure model. This module owns
//! what a page *looks like*: it mirrors the model into entities, and mirrors
//! player input back into the model. No page decision is taken here.

use bevy::prelude::*;
use bevy::text::FontSource;
use bevy::window::PrimaryWindow;

use crate::app_state::{AppState, AppTransitionRequests, AppTransitionSet, SettingsOrigin};
use crate::data::RulesData;
use crate::i18n::Localization;
use crate::input::{UIAction, UIActionEvent};
use crate::match_flow::{MatchResultSummary, MatchSeedSource, MatchSelection};
use crate::page::{CharacterSelectPage, MatchMode, PageCommand, PageItem, PageModel};
use crate::presentation::VirtualCanvas;

/// States that own a focusable page.
///
/// `Boot` has nothing to show and `Match` has no focus ring -- its pause input
/// is read directly by `client::input`, not through a focused item.
const PAGE_STATES: [AppState; 6] = [
    AppState::MainMenu,
    AppState::ModeSelect,
    AppState::CharacterSelect,
    AppState::Settings,
    AppState::Paused,
    AppState::Result,
];

/// The UI font, shipped with the build.
///
/// One weight covers both `zh-CN` and `en`; see `assets/README.md` for the
/// licence terms that keep the file unmodified.
const FONT_PATH: &str = "fonts/SourceHanSansCN-Bold.otf";

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        // Headless harnesses build the app without Bevy's UI stack. The page
        // model still runs there; only its rendering is absent, so the client
        // must not require a renderer to be exercised.
        if !app.is_plugin_added::<bevy::ui::UiPlugin>() {
            return;
        }

        app.init_resource::<UiFont>()
            .init_resource::<SelectedMode>()
            .add_systems(Startup, load_ui_font)
            .add_systems(Update, apply_virtual_canvas)
            .add_systems(
                Update,
                (drive_focused_page, drive_character_select).in_set(AppTransitionSet::Request),
            )
            .add_systems(PostUpdate, (refresh_focus_visuals, refresh_slot_visuals));

        for state in PAGE_STATES {
            app.add_systems(OnEnter(state), move |world: &mut World| {
                spawn_page(world, state);
            });
            // The page entities are state-scoped; the models mirroring them
            // have to leave with them, or a later page would drive a ring that
            // is no longer on screen.
            app.add_systems(OnExit(state), |mut commands: Commands| {
                commands.remove_resource::<ActivePage>();
                commands.remove_resource::<ActiveCharacterSelect>();
            });
        }
    }
}

/// Handle to the shipped UI font.
#[derive(Debug, Default, Resource)]
pub struct UiFont(pub Handle<Font>);

fn load_ui_font(mut font: ResMut<UiFont>, assets: Res<AssetServer>) {
    font.0 = assets.load(FONT_PATH);
}

/// The page currently on screen.
#[derive(Debug, Resource)]
pub struct ActivePage(pub PageModel);

/// Root of the current page's entities.
#[derive(Debug, Component)]
struct PageRoot;

/// One focusable row, tagged with the model item it mirrors.
#[derive(Debug, Component)]
struct PageItemNode(PageItem);

/// Scales the 1920x1080 design canvas into the real window.
///
/// The whole UI is authored in design pixels, so a single [`UiScale`] carries
/// the fit. `UiScale` only multiplies fixed lengths, which is why no page uses
/// percentage sizing: a percentage would ignore this factor and break the
/// letterbox.
fn apply_virtual_canvas(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut ui_scale: ResMut<UiScale>,
    mut roots: Query<&mut Node, With<PageRoot>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let layout = VirtualCanvas::layout(window.width(), window.height());
    ui_scale.0 = layout.ui_scale;

    // The letterbox offset is expressed in design pixels because `UiScale`
    // multiplies it again on the way to physical pixels.
    let (bar_x, bar_y) = layout.letterbox;
    for mut node in &mut roots {
        node.left = px(bar_x / layout.ui_scale);
        node.top = px(bar_y / layout.ui_scale);
    }
}

/// Localization key for an item's label.
const fn label_key(item: PageItem) -> &'static str {
    match item {
        PageItem::StartGame => "main_menu.start",
        PageItem::Settings => "main_menu.settings",
        PageItem::Exit => "main_menu.exit",
        PageItem::SinglePlayer => "mode_select.single_player",
        PageItem::LocalVersus => "mode_select.local_versus",
        PageItem::Lan => "mode_select.lan",
        PageItem::ConfirmCharacters => "character_select.confirm",
        PageItem::Back => "common.back",
        PageItem::Resume => "pause.resume",
        PageItem::Restart => "pause.restart",
        PageItem::ReturnToMainMenu => "common.return_to_main_menu",
        PageItem::Rematch => "result.rematch",
    }
}

/// Localization key for a page's heading.
const fn title_key(state: AppState) -> &'static str {
    match state {
        AppState::ModeSelect => "mode_select.title",
        AppState::CharacterSelect => "character_select.title",
        AppState::Settings => "settings.title",
        AppState::Paused => "pause.title",
        AppState::Result => "result.title",
        _ => "app.title",
    }
}

// Lab signal panel palette: dark ground, saturated accent, high-contrast text.
const GROUND: Color = Color::srgb(0.04, 0.05, 0.07);
const CHIP: Color = Color::srgb(0.10, 0.12, 0.16);
const CHIP_FOCUSED: Color = Color::srgb(0.16, 0.42, 0.55);
const TEXT: Color = Color::srgb(0.90, 0.94, 0.98);
const TEXT_DISABLED: Color = Color::srgb(0.45, 0.48, 0.53);

fn spawn_page(world: &mut World, state: AppState) {
    let origin = world.get_resource::<SettingsOrigin>().copied();
    let Some(model) = PageModel::for_state(state, origin) else {
        return;
    };
    let font = world.resource::<UiFont>().0.clone();
    let rows: Vec<(PageItem, String, bool)> = {
        let localization = world.resource::<Localization>();
        model
            .items()
            .iter()
            .map(|item| {
                let mut label = localization.text(label_key(item.id));
                if let Some(reason) = &item.unavailable_reason {
                    label.push_str(&format!("  ({reason})"));
                }
                (item.id, label, item.enabled)
            })
            .collect()
    };
    let title = world.resource::<Localization>().text(title_key(state));

    let root = world
        .spawn((
            PageRoot,
            DespawnOnExit(state),
            Node {
                position_type: PositionType::Absolute,
                width: px(1920),
                height: px(1080),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(18),
                ..default()
            },
            BackgroundColor(GROUND),
        ))
        .id();

    let heading = world
        .spawn((
            Text::new(title),
            TextFont {
                font: FontSource::Handle(font.clone()),
                font_size: FontSize::Px(64.0),
                ..default()
            },
            TextColor(TEXT),
            Node {
                margin: UiRect::bottom(px(40)),
                ..default()
            },
        ))
        .id();
    world.entity_mut(root).add_child(heading);

    match state {
        AppState::CharacterSelect => spawn_character_slots(world, root, &font),
        AppState::Result => spawn_scoreline(world, root, &font),
        _ => {}
    }

    for (id, label, enabled) in rows {
        let mut row = world.spawn((
            PageItemNode(id),
            Node {
                width: px(560),
                height: px(72),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(10)),
                ..default()
            },
            BorderColor::all(CHIP_FOCUSED),
            BackgroundColor(CHIP),
        ));
        if !enabled {
            // Grayed out but still focusable, so the player can read why.
            row.insert(bevy::ui::InteractionDisabled);
        }
        let row = row.id();

        let text = world
            .spawn((
                Text::new(label),
                TextFont {
                    font: FontSource::Handle(font.clone()),
                    font_size: FontSize::Px(32.0),
                    ..default()
                },
                TextColor(if enabled { TEXT } else { TEXT_DISABLED }),
            ))
            .id();
        world.entity_mut(row).add_child(text);
        world.entity_mut(root).add_child(row);
    }

    world.insert_resource(ActivePage(model));
}

/// Mode the player picked on the mode page.
///
/// It decides how many locals the character page gives a slot to, so it is
/// recorded when the mode item is confirmed rather than inferred later.
#[derive(Debug, Clone, Copy, Resource)]
pub struct SelectedMode(pub MatchMode);

impl Default for SelectedMode {
    fn default() -> Self {
        Self(MatchMode::SinglePlayer)
    }
}

/// The character page's two-slot model while it is on screen.
#[derive(Debug, Resource)]
pub struct ActiveCharacterSelect(pub CharacterSelectPage);

/// One character cell, tagged with the slot and roster index it shows.
#[derive(Debug, Component)]
struct CharacterCell {
    slot: usize,
    index: usize,
}

/// Characters this library can actually start a match with.
///
/// A character whose gameplay data failed to load narrows selection without
/// blocking the others, so the roster is filtered by whether play data exists.
fn selectable_characters(rules: &RulesData) -> Vec<game_core::config::CharacterId> {
    let Some(library) = rules.rules() else {
        return Vec::new();
    };
    let Some(profile) = library.profile_ids().next().cloned() else {
        return Vec::new();
    };
    library
        .roster()
        .characters
        .iter()
        .map(|identity| identity.id.clone())
        .filter(|id| library.character_play(&profile, id).is_some())
        .collect()
}

fn spawn_character_slots(world: &mut World, root: Entity, font: &Handle<Font>) {
    let characters = world
        .get_resource::<RulesData>()
        .map(selectable_characters)
        .unwrap_or_default();
    if characters.is_empty() {
        return;
    }
    let mode = world.resource::<SelectedMode>().0;

    let columns = world
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(120),
                margin: UiRect::bottom(px(40)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    world.entity_mut(root).add_child(columns);

    for slot in 0..2 {
        let column = world
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .id();
        world.entity_mut(columns).add_child(column);

        let heading = world
            .spawn((
                Text::new(if slot == 0 { "P1" } else { "P2" }),
                TextFont {
                    font: FontSource::Handle(font.clone()),
                    font_size: FontSize::Px(28.0),
                    ..default()
                },
                TextColor(TEXT),
            ))
            .id();
        world.entity_mut(column).add_child(heading);

        for (index, id) in characters.iter().enumerate() {
            let cell = world
                .spawn((
                    CharacterCell { slot, index },
                    Node {
                        width: px(320),
                        height: px(64),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(10)),
                        ..default()
                    },
                    BorderColor::all(CHIP),
                    BackgroundColor(CHIP),
                ))
                .id();
            let label = world
                .spawn((
                    Text::new(id.0.clone()),
                    TextFont {
                        font: FontSource::Handle(font.clone()),
                        font_size: FontSize::Px(26.0),
                        ..default()
                    },
                    TextColor(TEXT),
                ))
                .id();
            world.entity_mut(cell).add_child(label);
            world.entity_mut(column).add_child(cell);
        }
    }

    world.insert_resource(ActiveCharacterSelect(CharacterSelectPage::new(
        mode, characters,
    )));
}

/// Final score and winner under the result heading.
fn spawn_scoreline(world: &mut World, root: Entity, font: &Handle<Font>) {
    let Some(summary) = world.get_resource::<MatchResultSummary>().copied() else {
        return;
    };
    let winner = match summary.winner {
        Some(0) => "P1",
        Some(_) => "P2",
        None => "--",
    };
    let text = format!("{}  {} : {}", winner, summary.wins[0], summary.wins[1]);

    let scoreline = world
        .spawn((
            Text::new(text),
            TextFont {
                font: FontSource::Handle(font.clone()),
                font_size: FontSize::Px(48.0),
                ..default()
            },
            TextColor(TEXT),
            Node {
                margin: UiRect::bottom(px(40)),
                ..default()
            },
        ))
        .id();
    world.entity_mut(root).add_child(scoreline);
}

/// Feed player input into the page model and act on what it decides.
fn drive_focused_page(
    mut actions: MessageReader<UIActionEvent>,
    page: Option<ResMut<ActivePage>>,
    character_select: Option<Res<ActiveCharacterSelect>>,
    mut requests: ResMut<AppTransitionRequests>,
    mut origin: ResMut<SettingsOrigin>,
    mut exit: MessageWriter<AppExit>,
    mut mode: ResMut<SelectedMode>,
) {
    let Some(mut page) = page else {
        actions.clear();
        return;
    };
    for event in actions.read() {
        // On the character page the slots own direction and confirm: the row
        // ring must not also fire its confirm, or the page would leave for
        // `Match` before anyone had picked a character. Back still belongs to
        // the ring, which is what leaves the page.
        if character_select.is_some() && event.action != UIAction::Back {
            continue;
        }
        // The mode has to be read off the item that was confirmed: both mode
        // items lead to the same state, so the transition alone cannot say
        // which one the player chose.
        let confirmed = page.0.focused().id;
        let Some(command) = page.0.handle_player(event.player, event.action) else {
            continue;
        };
        match command {
            PageCommand::Transition(request) => {
                match confirmed {
                    PageItem::SinglePlayer => *mode = SelectedMode(MatchMode::SinglePlayer),
                    PageItem::LocalVersus => *mode = SelectedMode(MatchMode::LocalVersus),
                    _ => {}
                }
                // Recorded before the request so the settings page knows where
                // to return, whichever entry opened it.
                if request.target == AppState::Settings {
                    *origin = SettingsOrigin(page.0.state());
                }
                requests.submit(request.target, request.cause);
            }
            PageCommand::ExitApplication => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// Feed input into the character page's two slots.
///
/// The page's own `Confirm` picks a character for the acting slot. Only once
/// both slots hold one does a further `Confirm` commit the selection, which is
/// what makes the confirm item unavailable until then.
fn drive_character_select(
    mut actions: MessageReader<UIActionEvent>,
    page: Option<ResMut<ActiveCharacterSelect>>,
    rules: Option<Res<RulesData>>,
    mut seeds: ResMut<MatchSeedSource>,
    mut commands: Commands,
    mut selection_written: Local<bool>,
) {
    // The model exists only while its page is on screen, so its presence is
    // the page check.
    let Some(mut page) = page else {
        *selection_written = false;
        return;
    };
    for event in actions.read() {
        if event.action == UIAction::Confirm && page.0.confirm_enabled() && !*selection_written {
            let selected = page.0.selected();
            let (Some(first), Some(second)) = (selected[0], selected[1]) else {
                continue;
            };
            let Some(profile) = rules
                .as_deref()
                .and_then(RulesData::rules)
                .and_then(|library| library.profile_ids().next().cloned())
            else {
                continue;
            };
            // Only the selection is written here. Freezing it and requesting
            // the transition stay with `match_flow`, which owns both.
            commands.insert_resource(MatchSelection {
                rule_profile_id: profile,
                root_seed: seeds.next_seed(),
                characters: [first.clone(), second.clone()],
                confirmed: true,
            });
            *selection_written = true;
            continue;
        }
        if event.action == UIAction::Back {
            // Back leaves the page, which is the row ring's job.
            continue;
        }
        page.0.handle_player(event.player, event.action);
    }
}

/// Mirror each slot's focus and selection onto its cells.
fn refresh_slot_visuals(
    page: Option<Res<ActiveCharacterSelect>>,
    mut cells: Query<(&CharacterCell, &mut BackgroundColor, &mut BorderColor)>,
) {
    let Some(page) = page else {
        return;
    };
    let selected = page.0.selected();
    for (cell, mut background, mut border) in &mut cells {
        let focused = page.0.focused_index(cell.slot) == Some(cell.index);
        background.0 = if focused { CHIP_FOCUSED } else { CHIP };
        // A confirmed slot keeps a lit border, so focus and commitment stay
        // distinguishable after the cursor moves on.
        let committed = selected
            .get(cell.slot)
            .and_then(Option::as_ref)
            .is_some_and(|id| page.0.characters().get(cell.index) == Some(id));
        *border = BorderColor::all(if committed { CHIP_FOCUSED } else { CHIP });
    }
}

/// Mirror the model's focus onto the rows.
fn refresh_focus_visuals(
    page: Option<Res<ActivePage>>,
    character_select: Option<Res<ActiveCharacterSelect>>,
    mut rows: Query<(&PageItemNode, &mut BackgroundColor, &Children)>,
    mut text_colors: Query<&mut TextColor>,
) {
    let Some(page) = page else {
        return;
    };
    let focused = page.0.focused().id;
    // The confirm row stays unavailable until both slots hold a character.
    let awaiting_slots = character_select
        .as_deref()
        .is_some_and(|select| !select.0.confirm_enabled());

    for (item, mut background, children) in &mut rows {
        let enabled = !(item.0 == PageItem::ConfirmCharacters && awaiting_slots);
        background.0 = match (enabled, item.0 == focused) {
            (false, _) => CHIP,
            (true, true) => CHIP_FOCUSED,
            (true, false) => CHIP,
        };
        for child in children.iter() {
            if let Ok(mut color) = text_colors.get_mut(child) {
                color.0 = if enabled { TEXT } else { TEXT_DISABLED };
            }
        }
    }
}
