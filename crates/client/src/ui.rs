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
use crate::match_flow::{MatchResultSummary, MatchSeedSource, MatchSelection, SelectedMode};
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
            .add_systems(Startup, load_ui_font)
            .add_systems(Update, apply_virtual_canvas)
            .add_systems(Update, drive_focused_page.in_set(AppTransitionSet::Request))
            .add_systems(Update, complete_binding_capture)
            .add_systems(
                PostUpdate,
                (
                    refresh_focus_visuals,
                    refresh_slot_visuals,
                    refresh_page_text,
                    refresh_setting_values,
                    refresh_key_legend,
                    refresh_binding_rejection,
                ),
            );

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

/// The text naming one row, tagged so a language change can rewrite it.
#[derive(Debug, Component)]
struct PageItemLabel(PageItem);

/// The page heading, which a language change rewrites from the current state.
#[derive(Debug, Component)]
struct PageTitle;

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
        PageItem::AiVersus => "mode_select.ai_versus",
        PageItem::Lan => "mode_select.lan",
        PageItem::ConfirmCharacters => "character_select.confirm",
        PageItem::Back => "common.back",
        PageItem::Resume => "pause.resume",
        PageItem::Restart => "pause.restart",
        PageItem::ReturnToMainMenu => "common.return_to_main_menu",
        PageItem::Rematch => "result.rematch",
        PageItem::Language => "settings.language",
        PageItem::WindowMode => "settings.window_mode",
        PageItem::MasterVolume => "settings.master_volume",
        PageItem::SfxVolume => "settings.sfx_volume",
        PageItem::Vibration => "settings.vibration",
        PageItem::AnimationIntensity => "settings.animation_intensity",
        PageItem::ColorAssist => "settings.color_assist",
        PageItem::Rebind { .. } => "settings.rebind",
    }
}

/// The text naming one row, in the current language.
///
/// An item the page refuses to enable says why in the same line, so the reason
/// travels with the name instead of needing a place of its own.
#[must_use]
pub fn item_label(localization: &Localization, item: &crate::page::FocusItem) -> String {
    let mut label = match item.id {
        PageItem::Rebind {
            player,
            action,
            device,
        } => rebind_label(localization, player, action, device),
        id => localization.text(label_key(id)),
    };
    if let Some(reason) = &item.unavailable_reason {
        label.push_str(&format!("  ({})", localization.text(reason)));
    }
    label
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
/// Ground for a page that overlays a live match, dimming it without hiding it.
const SCRIM: Color = Color::srgba(0.04, 0.05, 0.07, 0.82);
const CHIP: Color = Color::srgb(0.10, 0.12, 0.16);
const CHIP_FOCUSED: Color = Color::srgb(0.16, 0.42, 0.55);
const TEXT: Color = Color::srgb(0.90, 0.94, 0.98);
const TEXT_DISABLED: Color = Color::srgb(0.45, 0.48, 0.53);
/// Refusals and other "that did not happen" reports.
const WARNING: Color = Color::srgb(0.90, 0.55, 0.30);

/// Pages that sit over a running match dim it instead of replacing it, so the
/// board stays readable underneath.
const fn page_ground(state: AppState, origin: Option<SettingsOrigin>) -> Color {
    match state {
        AppState::Paused => SCRIM,
        AppState::Settings => match origin {
            Some(SettingsOrigin(AppState::Paused)) => SCRIM,
            _ => GROUND,
        },
        _ => GROUND,
    }
}

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
            .map(|item| (item.id, item_label(localization, item), item.enabled))
            .collect()
    };
    let title = world.resource::<Localization>().text(title_key(state));

    let root = world
        .spawn((
            PageRoot,
            DespawnOnExit(state),
            // Above the HUD, so a page opened over a running match covers it.
            GlobalZIndex(10),
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
            BackgroundColor(page_ground(state, origin)),
        ))
        .id();

    let heading = world
        .spawn((
            PageTitle,
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

    spawn_key_legend(world, root, &font);

    match state {
        AppState::CharacterSelect => spawn_character_slots(world, root, &font),
        AppState::Result => spawn_scoreline(world, root, &font),
        AppState::Settings => {
            spawn_settings_rows(world, root, &font, &rows);
            world.insert_resource(ActivePage(model));
            return;
        }
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

/// The text showing a setting's current value.
#[derive(Debug, Component)]
struct SettingValueText(PageItem);

/// Localization key naming a configurable rules action.
const fn action_key(action: game_core::input::GameAction) -> &'static str {
    use game_core::input::GameAction;
    match action {
        GameAction::SoftDrop => "action.soft_drop",
        GameAction::HardDrop => "action.hard_drop",
        GameAction::RotateClockwise => "action.rotate_cw",
        GameAction::RotateCounterClockwise => "action.rotate_ccw",
        GameAction::Left => "action.left",
        GameAction::Right => "action.right",
    }
}

/// Localization key naming a menu action.
const fn ui_action_key(action: UIAction) -> &'static str {
    match action {
        UIAction::Confirm => "ui_action.confirm",
        UIAction::Back => "ui_action.back",
        UIAction::Left => "action.left",
        UIAction::Right => "action.right",
        UIAction::Up => "ui_action.up",
        UIAction::Down => "ui_action.down",
    }
}

/// How a bound physical input reads on screen.
///
/// Bindings persist Bevy's own variant spelling, which is precise but not what
/// a player sees printed on the key. The legend is only useful if the string it
/// shows is the one on the keycap, so the common families are shortened and
/// anything else falls through unchanged.
#[must_use]
pub fn key_display(name: &str) -> String {
    for prefix in ["Key", "Digit"] {
        if let Some(rest) = name.strip_prefix(prefix)
            && rest.len() == 1
        {
            return rest.to_owned();
        }
    }
    match name {
        "ArrowUp" => "↑".to_owned(),
        "ArrowDown" => "↓".to_owned(),
        "ArrowLeft" => "←".to_owned(),
        "ArrowRight" => "→".to_owned(),
        _ => name
            .strip_prefix("Numpad")
            .map_or_else(|| name.to_owned(), |rest| format!("Num{rest}")),
    }
}

/// How a setting currently reads on the page.
/// The value shown beside one setting's name, in the current language.
///
/// Numbers stay numbers; everything a player picks from a fixed set is named in
/// their own language, because the value is as much a word as the name is.
#[must_use]
pub fn setting_value(
    item: PageItem,
    settings: &crate::settings::UserSettings,
    localization: &Localization,
) -> String {
    use crate::settings::{AnimationIntensity, WindowModeSetting};
    let switch = |on: bool| localization.text(if on { "settings.on" } else { "settings.off" });
    match item {
        PageItem::Language => language_name(localization, &settings.language),
        PageItem::WindowMode => localization.text(match settings.window_mode {
            WindowModeSetting::Windowed => "settings.window_mode.windowed",
            WindowModeSetting::BorderlessFullscreen => "settings.window_mode.borderless",
            WindowModeSetting::Fullscreen => "settings.window_mode.fullscreen",
        }),
        PageItem::MasterVolume => format!("{:.0}%", settings.master_volume * 100.0),
        PageItem::SfxVolume => format!("{:.0}%", settings.sfx_volume * 100.0),
        PageItem::Vibration => switch(settings.vibration),
        PageItem::AnimationIntensity => localization.text(match settings.animation_intensity {
            AnimationIntensity::Full => "settings.animation_intensity.full",
            AnimationIntensity::Reduced => "settings.animation_intensity.reduced",
        }),
        PageItem::ColorAssist => switch(settings.color_assist),
        PageItem::Rebind {
            player,
            action,
            device,
        } => settings
            .players
            .get(player)
            .and_then(|bindings| bindings.input_for(action, device))
            .map_or_else(|| "--".to_owned(), |input| key_display(input.name())),
        _ => String::new(),
    }
}

/// A language's own name for itself.
///
/// Every catalog spells every language the same way, so the list reads the same
/// whichever language is currently selected -- which is the point: a player who
/// cannot read the current one still has to find their own. A locale no catalog
/// names falls back to its code, which is at least selectable.
#[must_use]
pub fn language_name(localization: &Localization, code: &str) -> String {
    let key = format!("language.{code}");
    let name = localization.text(&key);
    if name == key { code.to_owned() } else { name }
}

/// Label for a rebinding row, which names a player, action and device.
///
/// The two rotations also carry the menu confirm and back, so their rows name
/// both meanings: one key, one row, both of the things it does.
#[must_use]
pub fn rebind_label(
    localization: &Localization,
    player: usize,
    action: game_core::input::GameAction,
    device: crate::settings::DeviceCategory,
) -> String {
    use crate::settings::DeviceCategory;
    let device = match device {
        DeviceCategory::Keyboard => "KB",
        DeviceCategory::Gamepad => "PAD",
    };
    let mut name = localization.text(action_key(action));
    if let Some(ui_action) = crate::input::BOUND_UI_ACTIONS
        .into_iter()
        .find(|candidate| crate::input::ui_action_source(*candidate) == Some(action))
    {
        name.push_str(" / ");
        name.push_str(&localization.text(ui_action_key(ui_action)));
    }
    format!("P{} {name} [{device}]", player + 1)
}

/// The settings page: general settings in one column, rebindings in the other.
///
/// Two columns because the page lists more rows than a 1080-tall canvas fits in
/// one. The focus ring stays a single linear ring across both.
fn spawn_settings_rows(
    world: &mut World,
    root: Entity,
    font: &Handle<Font>,
    rows: &[(PageItem, String, bool)],
) {
    let settings = world.resource::<crate::settings::UserSettings>().clone();
    // Resolved before the first spawn: writing entities needs the world
    // exclusively, and the values need the catalogs while they are still
    // borrowable.
    let values: Vec<(PageItem, String)> = {
        let localization = world.resource::<Localization>();
        rows.iter()
            .filter(|(id, _, _)| id.is_setting())
            .map(|(id, _, _)| (*id, setting_value(*id, &settings, localization)))
            .collect()
    };

    let columns = world
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(80),
                align_items: AlignItems::FlexStart,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    world.entity_mut(root).add_child(columns);

    let mut column_ids = Vec::new();
    for _ in 0..2 {
        let column = world
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .id();
        world.entity_mut(columns).add_child(column);
        column_ids.push(column);
    }

    for (id, label, _) in rows {
        let is_rebind = matches!(id, PageItem::Rebind { .. });
        let parent = column_ids[usize::from(is_rebind)];
        let label = label.clone();

        let row = world
            .spawn((
                PageItemNode(*id),
                Node {
                    width: px(520),
                    height: px(40),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::horizontal(px(16)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BorderColor::all(CHIP),
                BackgroundColor(CHIP),
            ))
            .id();

        let name = world
            .spawn((
                PageItemLabel(*id),
                Text::new(label),
                TextFont {
                    font: FontSource::Handle(font.clone()),
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(TEXT),
            ))
            .id();
        world.entity_mut(row).add_child(name);

        if id.is_setting() {
            let value = world
                .spawn((
                    SettingValueText(*id),
                    Text::new(
                        values
                            .iter()
                            .find_map(|(candidate, value)| (candidate == id).then(|| value.clone()))
                            .unwrap_or_default(),
                    ),
                    TextFont {
                        font: FontSource::Handle(font.clone()),
                        font_size: FontSize::Px(22.0),
                        ..default()
                    },
                    TextColor(CHIP_FOCUSED),
                ))
                .id();
            world.entity_mut(row).add_child(value);
        }
        world.entity_mut(parent).add_child(row);
    }

    // A refused rebinding changes nothing else on the page, so without this the
    // player's key press would look like it was simply ignored.
    let notice = world
        .spawn((
            BindingRejectionText,
            Text::new(String::new()),
            TextFont {
                font: FontSource::Handle(font.clone()),
                font_size: FontSize::Px(22.0),
                ..default()
            },
            TextColor(WARNING),
            Node {
                margin: UiRect::top(px(24)),
                ..default()
            },
        ))
        .id();
    world.entity_mut(root).add_child(notice);
}

/// The line explaining why the last rebinding was refused.
#[derive(Debug, Component)]
struct BindingRejectionText;

/// Report a refused rebinding, naming the key and the action that holds it.
fn refresh_binding_rejection(
    rejection: Res<crate::settings::LastBindingRejection>,
    localization: Res<Localization>,
    mut notices: Query<&mut Text, With<BindingRejectionText>>,
) {
    let shown = rejection.0.as_ref().map_or_else(String::new, |conflict| {
        format!(
            "{} {} → {}",
            localization.text("settings.binding_taken"),
            key_display(conflict.input.name()),
            localization.text(action_key(conflict.existing)),
        )
    });
    for mut text in &mut notices {
        if text.0 != shown {
            text.0.clone_from(&shown);
        }
    }
}

/// One local player's confirm/back legend, pinned to a bottom corner.
#[derive(Debug, Component)]
struct KeyLegend(usize);

/// The confirm and back keys a player currently holds, named on screen.
///
/// Every menu page carries it because confirm and back follow the player's
/// rotation bindings: after a rebinding the keys are whatever that player chose,
/// and the only way to know without opening the settings page is to be told
/// here. P1 reads the left corner and P2 the right, which is also the side each
/// one plays on.
fn spawn_key_legend(world: &mut World, root: Entity, font: &Handle<Font>) {
    for player in 0..crate::input::LOCAL_PLAYERS {
        let mut node = Node {
            position_type: PositionType::Absolute,
            bottom: px(32),
            ..default()
        };
        if player == 0 {
            node.left = px(56);
        } else {
            node.right = px(56);
        }

        let legend = world
            .spawn((
                KeyLegend(player),
                // Filled by `refresh_key_legend`, which also keeps it current
                // when a rebinding or a gamepad changes what the keys are.
                Text::new(String::new()),
                TextFont {
                    font: FontSource::Handle(font.clone()),
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(TEXT_DISABLED),
                node,
            ))
            .id();
        world.entity_mut(root).add_child(legend);
    }
}

/// How one player's legend reads right now.
#[must_use]
pub fn key_legend_text(
    player: usize,
    settings: &crate::settings::UserSettings,
    slots: &crate::input::GamepadSlots,
    localization: &Localization,
) -> String {
    use crate::settings::DeviceCategory;

    let Some(bindings) = settings.players.get(player) else {
        return String::new();
    };
    // A player holding a pad is not pressing keyboard keys, so the legend names
    // the device in their hands rather than listing both.
    let device = if slots.pad(player).is_some() {
        DeviceCategory::Gamepad
    } else {
        DeviceCategory::Keyboard
    };

    let mut parts = vec![format!("P{}", player + 1)];
    for action in crate::input::BOUND_UI_ACTIONS {
        let Some(source) = crate::input::ui_action_source(action) else {
            continue;
        };
        let key = bindings
            .input_for(source, device)
            .map_or_else(|| "--".to_owned(), |input| key_display(input.name()));
        parts.push(format!(
            "{key} {}",
            localization.text(ui_action_key(action))
        ));
    }
    parts.join("    ")
}

/// Keep each corner legend equal to the bindings actually in force.
fn refresh_key_legend(
    settings: Res<crate::settings::UserSettings>,
    slots: Res<crate::input::GamepadSlots>,
    localization: Res<Localization>,
    mut legends: Query<(&KeyLegend, &mut Text)>,
) {
    for (legend, mut text) in &mut legends {
        let shown = key_legend_text(legend.0, &settings, &slots, &localization);
        if text.0 != shown {
            text.0 = shown;
        }
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
    mut pages: PageDrivers,
    mut requests: ResMut<AppTransitionRequests>,
    mut origin: ResMut<SettingsOrigin>,
    mut exit: MessageWriter<AppExit>,
    mut mode: ResMut<SelectedMode>,
) {
    // The one-shot guard belongs to the character page, so it resets whenever
    // that page is not up.
    if pages.character_select.is_none() {
        *pages.selection_written = false;
    }
    let Some(mut page) = page else {
        actions.clear();
        return;
    };
    // One reader for the whole page stack. Bevy keeps a message readable for
    // two frames, so a second system reading the same stream would see the
    // confirm that opened a page again once that page was up -- and act on it.
    for event in actions.read() {
        if pages.character_select.is_some() {
            // On the character page the slots own direction and confirm: the
            // row ring must not also fire its confirm, or the page would leave
            // for `Match` before anyone had picked a character. Back still
            // belongs to the ring, which is what leaves the page.
            if event.action != UIAction::Back {
                drive_character_select(&mut pages, event);
                continue;
            }
        } else if page.0.state() == AppState::Settings
            && event.action == UIAction::Confirm
            // Only a setting is edited in place. A navigation row on the same
            // page -- `Back` -- still belongs to the ring, or the page would
            // swallow its own exit and strand the player in settings.
            && page.0.focused().id.is_setting()
        {
            drive_settings(&mut pages, page.0.focused().id);
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
                    PageItem::AiVersus => *mode = SelectedMode(MatchMode::AiVersus),
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

/// Everything the page drivers write to, kept in one bundle so the dispatcher
/// stays a single system with a single message cursor.
#[derive(bevy::ecs::system::SystemParam)]
struct PageDrivers<'w, 's> {
    character_select: Option<ResMut<'w, ActiveCharacterSelect>>,
    settings: ResMut<'w, crate::settings::UserSettings>,
    save: MessageWriter<'w, crate::settings::SaveSettingsRequest>,
    rules: Option<Res<'w, RulesData>>,
    seeds: ResMut<'w, MatchSeedSource>,
    rejection: ResMut<'w, crate::settings::LastBindingRejection>,
    commands: Commands<'w, 's>,
    selection_written: Local<'s, bool>,
}

/// Confirming a settings row edits it in place.
///
/// Confirm cycles the value rather than Left/Right adjusting it, because all
/// four directions move focus in the page model; giving two of them a second
/// meaning would contradict the focus contract every other page follows.
fn drive_settings(pages: &mut PageDrivers, focused: PageItem) {
    use crate::settings::{AnimationIntensity, BindingCapture, WindowModeSetting};

    if let PageItem::Rebind {
        player,
        action,
        device,
    } = focused
    {
        if let Some(capture) = BindingCapture::open(player, action, device) {
            // The refusal on screen belongs to the attempt that produced it, so
            // opening the next capture clears it.
            pages.rejection.0 = None;
            pages.commands.insert_resource(capture);
        }
        return;
    }

    let settings = &mut *pages.settings;
    match focused {
        PageItem::Language => {
            settings.language = if settings.language == "en" {
                "zh-CN".into()
            } else {
                "en".into()
            };
        }
        PageItem::WindowMode => {
            settings.window_mode = match settings.window_mode {
                WindowModeSetting::Windowed => WindowModeSetting::BorderlessFullscreen,
                WindowModeSetting::BorderlessFullscreen => WindowModeSetting::Fullscreen,
                WindowModeSetting::Fullscreen => WindowModeSetting::Windowed,
            };
        }
        PageItem::MasterVolume => settings.master_volume = next_volume(settings.master_volume),
        PageItem::SfxVolume => settings.sfx_volume = next_volume(settings.sfx_volume),
        PageItem::Vibration => settings.vibration = !settings.vibration,
        PageItem::ColorAssist => settings.color_assist = !settings.color_assist,
        PageItem::AnimationIntensity => {
            settings.animation_intensity = match settings.animation_intensity {
                AnimationIntensity::Full => AnimationIntensity::Reduced,
                AnimationIntensity::Reduced => AnimationIntensity::Full,
            };
        }
        _ => return,
    }
    // Persisted as soon as it is edited: the design has no separate commit
    // step, and a setting that took effect but was never written would come
    // back changed after a restart.
    pages.save.write(crate::settings::SaveSettingsRequest);
}

/// Volume steps, wrapping back to silence after full.
fn next_volume(current: f32) -> f32 {
    let stepped = (current * 10.0).round() as i32 + 1;
    if stepped > 10 {
        0.0
    } else {
        stepped as f32 / 10.0
    }
}

/// Complete an open rebinding from the next physical input.
///
/// This reads devices directly because a capture suspends the normal
/// `UIAction` path: the whole point is that the key being bound must not also
/// act as a menu action while it is being captured.
fn complete_binding_capture(
    capture: Option<Res<crate::settings::BindingCapture>>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut settings: ResMut<crate::settings::UserSettings>,
    mut rejection: ResMut<crate::settings::LastBindingRejection>,
    mut save: MessageWriter<crate::settings::SaveSettingsRequest>,
    mut commands: Commands,
) {
    use crate::input::PhysicalInput;
    use crate::settings::CaptureOutcome;

    let Some(capture) = capture else {
        return;
    };
    // Back cancels, leaving the original binding in place.
    if keys.just_pressed(KeyCode::Escape) {
        commands.remove_resource::<crate::settings::BindingCapture>();
        return;
    }

    let captured = keys
        .get_just_pressed()
        .next()
        .map(|code| PhysicalInput::keyboard(format!("{code:?}")))
        .or_else(|| {
            pads.iter().find_map(|pad| {
                pad.get_just_pressed()
                    .next()
                    .map(|button| PhysicalInput::gamepad(format!("{button:?}")))
            })
        });
    let Some(input) = captured else {
        return;
    };

    match capture.offer(&mut settings, &input) {
        Ok(CaptureOutcome::Ignored) => return,
        // A taken input is refused rather than moved. Taking it would leave its
        // previous action unbound, and an unbound rotation is also an unbound
        // menu confirm or back -- the player would lose the ability to leave
        // this page. The page reports which action holds the key instead.
        Ok(CaptureOutcome::Conflict(conflict)) => {
            commands.remove_resource::<crate::settings::BindingCapture>();
            rejection.0 = Some(conflict);
            return;
        }
        Ok(_) => {}
        Err(error) => warn!("rebinding failed: {error}"),
    }
    commands.remove_resource::<crate::settings::BindingCapture>();
    rejection.0 = None;
    save.write(crate::settings::SaveSettingsRequest);
}

/// Keep the value column showing what the settings actually hold.
fn refresh_setting_values(
    settings: Res<crate::settings::UserSettings>,
    localization: Res<Localization>,
    capture: Option<Res<crate::settings::BindingCapture>>,
    mut values: Query<(&SettingValueText, &mut Text)>,
) {
    for (value, mut text) in &mut values {
        let awaiting = capture.as_deref().is_some_and(|capture| {
            matches!(value.0, PageItem::Rebind { player, action, device }
                if player == capture.player && action == capture.action && device == capture.device)
        });
        let shown = if awaiting {
            "...".to_owned()
        } else {
            setting_value(value.0, &settings, &localization)
        };
        if text.0 != shown {
            text.0 = shown;
        }
    }
}

/// Rewrite every baked page string after a language change.
///
/// Row names and the heading are written once at spawn, which is enough for
/// every change except the one that alters all of them at once. Switching the
/// language must not require leaving and re-entering the page to take effect.
fn refresh_page_text(
    localization: Res<Localization>,
    state: Res<State<AppState>>,
    origin: Option<Res<SettingsOrigin>>,
    mut titles: Query<&mut Text, (With<PageTitle>, Without<PageItemLabel>)>,
    mut labels: Query<(&PageItemLabel, &mut Text), Without<PageTitle>>,
) {
    if !localization.is_changed() {
        return;
    }
    let state = *state.get();
    let Some(model) = PageModel::for_state(state, origin.as_deref().copied()) else {
        return;
    };

    let title = localization.text(title_key(state));
    for mut text in &mut titles {
        if text.0 != title {
            text.0.clone_from(&title);
        }
    }
    for (label, mut text) in &mut labels {
        let Some(item) = model.items().iter().find(|item| item.id == label.0) else {
            continue;
        };
        let shown = item_label(&localization, item);
        if text.0 != shown {
            text.0 = shown;
        }
    }
}

/// Feed input into the character page's two slots.
///
/// The page's own `Confirm` picks a character for the acting slot. Only once
/// both slots hold one does a further `Confirm` commit the selection, which is
/// what makes the confirm item unavailable until then.
fn drive_character_select(pages: &mut PageDrivers, event: &UIActionEvent) {
    let Some(page) = pages.character_select.as_mut() else {
        return;
    };
    if event.action == UIAction::Confirm && page.0.confirm_enabled() && !*pages.selection_written {
        let selected = page.0.selected();
        let (Some(first), Some(second)) = (selected[0], selected[1]) else {
            return;
        };
        let Some(profile) = pages
            .rules
            .as_deref()
            .and_then(RulesData::rules)
            .and_then(|library| library.profile_ids().next().cloned())
        else {
            return;
        };
        // Only the selection is written here. Freezing it and requesting the
        // transition stay with `match_flow`, which owns both.
        pages.commands.insert_resource(MatchSelection {
            rule_profile_id: profile,
            root_seed: pages.seeds.next_seed(),
            characters: [first.clone(), second.clone()],
            confirmed: true,
        });
        *pages.selection_written = true;
        return;
    }
    page.0.handle_player(event.player, event.action);
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
    mut text_colors: Query<(&mut TextColor, Has<SettingValueText>)>,
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
            // The value column carries its own accent colour to separate the
            // setting's name from what it is set to; focus does not own it.
            if let Ok((mut color, is_value)) = text_colors.get_mut(child)
                && !is_value
            {
                color.0 = if enabled { TEXT } else { TEXT_DISABLED };
            }
        }
    }
}
