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
use crate::i18n::Localization;
use crate::input::UIActionEvent;
use crate::page::{PageCommand, PageItem, PageModel};
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
            .add_systems(PostUpdate, refresh_focus_visuals);

        for state in PAGE_STATES {
            app.add_systems(OnEnter(state), move |world: &mut World| {
                spawn_page(world, state);
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

/// Feed player input into the page model and act on what it decides.
fn drive_focused_page(
    mut actions: MessageReader<UIActionEvent>,
    page: Option<ResMut<ActivePage>>,
    mut requests: ResMut<AppTransitionRequests>,
    mut origin: ResMut<SettingsOrigin>,
    mut exit: MessageWriter<AppExit>,
    state: Res<State<AppState>>,
) {
    let Some(mut page) = page else {
        actions.clear();
        return;
    };
    for event in actions.read() {
        let Some(command) = page.0.handle_player(event.player, event.action) else {
            continue;
        };
        match command {
            PageCommand::Transition(request) => {
                // Recorded before the request so the settings page knows where
                // to return, whichever entry opened it.
                if request.target == AppState::Settings {
                    *origin = SettingsOrigin(*state.get());
                }
                requests.submit(request.target, request.cause);
            }
            PageCommand::ExitApplication => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// Mirror the model's focus onto the rows.
fn refresh_focus_visuals(
    page: Option<Res<ActivePage>>,
    mut rows: Query<(&PageItemNode, &mut BackgroundColor)>,
) {
    let Some(page) = page else {
        return;
    };
    let focused = page.0.focused().id;
    for (item, mut background) in &mut rows {
        background.0 = if item.0 == focused {
            CHIP_FOCUSED
        } else {
            CHIP
        };
    }
}
