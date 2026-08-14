//! The in-match HUD: the resident half of the presentation layer.
//!
//! Everything here is rebuilt from the latest snapshot, never accumulated from
//! events. Entities are spawned once per match instance and then only have
//! their values written, so a high-feedback tick does not churn the ECS.

use bevy::prelude::*;
use bevy::text::FontSource;
use game_core::board::{Cell, Coord};

use crate::app_state::AppState;
use crate::match_flow::MatchInstanceId;
use crate::presentation::{MatchPresentationSnapshot, build_snapshot};
use crate::settings::UserSettings;
use crate::simulation::{LatestStepReport, RulesSimulation};
use crate::ui::UiFont;

/// Visible board rows. Hidden rows stay out of the HUD by definition.
const VISIBLE_ROWS: u8 = 12;
const BOARD_COLUMNS: u8 = 6;
const CELL: f32 = 44.0;
const CELL_GAP: f32 = 2.0;

const GROUND: Color = Color::srgb(0.04, 0.05, 0.07);
const PANEL: Color = Color::srgb(0.09, 0.11, 0.14);
const GRID: Color = Color::srgb(0.13, 0.15, 0.19);
const TEXT: Color = Color::srgb(0.90, 0.94, 0.98);
const DANGER: Color = Color::srgb(0.85, 0.28, 0.24);

/// Ball colours, indexed by the rule data's colour id.
const BALL_COLORS: [Color; 5] = [
    Color::srgb(0.87, 0.29, 0.33),
    Color::srgb(0.36, 0.72, 0.36),
    Color::srgb(0.30, 0.55, 0.88),
    Color::srgb(0.90, 0.78, 0.30),
    Color::srgb(0.70, 0.44, 0.85),
];

/// A non-colour cue per colour id, so colour is never the only signal.
const BALL_GLYPHS: [&str; 5] = ["●", "▲", "■", "◆", "★"];

const NUISANCE_COLOR: Color = Color::srgb(0.42, 0.44, 0.48);
const NUISANCE_GLYPH: &str = "✕";

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy::ui::UiPlugin>() {
            return;
        }
        app.add_systems(Update, (spawn_hud, release_hud, refresh_hud).chain());
    }
}

/// Root of the HUD, tagged with the instance it belongs to.
#[derive(Debug, Component)]
struct HudRoot(MatchInstanceId);

/// One board cell.
#[derive(Debug, Component)]
struct BoardCell {
    slot: usize,
    column: u8,
    /// Visible row, counted from the top.
    row: u8,
}

/// A HUD text whose content is written from the snapshot.
#[derive(Debug, Component)]
enum HudText {
    Score(usize),
    PendingGarbage(usize),
    FeverGarbage(usize),
    FeverGauge(usize),
    FeverTime(usize),
    FeverTarget(usize),
    Chain(usize),
    Next { slot: usize, index: usize },
    Character(usize),
    Scoreline,
    Phase,
}

/// The HUD lives exactly as long as the rules instance it shows.
///
/// Binding it to the instance rather than to `AppState::Match` is what keeps
/// the board on screen while the pause and settings pages sit over it.
fn spawn_hud(
    instance: Option<Res<MatchInstanceId>>,
    existing: Query<&HudRoot>,
    font: Res<UiFont>,
    mut commands: Commands,
) {
    let Some(instance) = instance else {
        return;
    };
    if existing.iter().any(|root| root.0 == *instance) {
        return;
    }
    build_hud(&mut commands, *instance, &font.0);
}

fn release_hud(
    instance: Option<Res<MatchInstanceId>>,
    roots: Query<(Entity, &HudRoot)>,
    mut commands: Commands,
) {
    for (entity, root) in &roots {
        if instance.as_deref().is_none_or(|current| *current != root.0) {
            commands.entity(entity).despawn();
        }
    }
}

/// Write the latest snapshot onto the HUD.
fn refresh_hud(
    state: Res<State<AppState>>,
    simulation: Option<Res<RulesSimulation>>,
    report: Res<LatestStepReport>,
    settings: Res<UserSettings>,
    mut cells: Query<(
        &BoardCell,
        &mut BackgroundColor,
        &mut BorderColor,
        &Children,
    )>,
    mut texts: Query<(&HudText, &mut Text)>,
    mut glyphs: Query<&mut Text, Without<HudText>>,
) {
    // The pause and settings pages sit over a live board, so the HUD keeps
    // showing the last snapshot rather than blanking while they are up.
    if !matches!(
        state.get(),
        AppState::Match | AppState::Paused | AppState::Settings
    ) {
        return;
    }
    let Some(simulation) = simulation else {
        return;
    };
    let view = simulation.0.view();
    let Some(snapshot) = build_snapshot(
        Some(&view),
        report.0.as_ref(),
        simulation.0.spec(),
        settings.animation_intensity,
    ) else {
        return;
    };

    for (cell, mut background, mut border, children) in &mut cells {
        let player = &snapshot.players[cell.slot];
        let occupant = cell_occupant(player, cell);
        let (color, glyph) = match occupant {
            Cell::Empty => (GRID, ""),
            Cell::Color(id) => {
                let index = usize::from(id) % BALL_COLORS.len();
                (BALL_COLORS[index], BALL_GLYPHS[index])
            }
            Cell::Nuisance => (NUISANCE_COLOR, NUISANCE_GLYPH),
        };
        background.0 = color;
        // The top visible row is the overflow line; marking it keeps the danger
        // readable without relying on the ball colours.
        *border = BorderColor::all(if cell.row == 0 && player.overflow_risk {
            DANGER
        } else {
            GRID
        });
        for child in children.iter() {
            if let Ok(mut text) = glyphs.get_mut(child)
                && text.0 != glyph
            {
                text.0 = glyph.to_owned();
            }
        }
    }

    for (field, mut text) in &mut texts {
        let value = hud_value(field, &snapshot);
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// What occupies one visible cell: the settled board, or the active group.
fn cell_occupant(
    player: &crate::presentation::PlayerPresentationSnapshot,
    cell: &BoardCell,
) -> Cell {
    let geometry = player.board.geometry();
    let y = geometry.hidden_rows() + cell.row;
    let Some(coord) = Coord::new(cell.column, y) else {
        return Cell::Empty;
    };
    if let Some(group) = player.active_drop.as_ref()
        && let Some(cells) = group.cells(&player.board)
        && let Some((_, color)) = cells.iter().find(|(at, _)| *at == coord)
    {
        return Cell::Color(*color);
    }
    player.board.get(coord)
}

fn hud_value(field: &HudText, snapshot: &MatchPresentationSnapshot) -> String {
    let player = |slot: usize| &snapshot.players[slot];
    match *field {
        HudText::Score(slot) => format!("{}", player(slot).score),
        HudText::PendingGarbage(slot) => format!("{}", player(slot).pending_garbage),
        HudText::FeverGarbage(slot) => format!("{}", player(slot).fever_garbage),
        HudText::FeverGauge(slot) => format!("{} / 7", player(slot).fever_gauge),
        HudText::FeverTime(slot) => {
            let seconds = player(slot).fever_time_ticks / 60;
            if player(slot).fever_state {
                format!("FEVER {seconds}s")
            } else {
                "--".to_owned()
            }
        }
        HudText::FeverTarget(slot) => player(slot)
            .fever_target
            .map_or_else(|| "--".to_owned(), |target| format!("TARGET {target}")),
        HudText::Chain(slot) => {
            let chain = player(slot).chain_count;
            if chain > 0 {
                format!("CHAIN {chain}")
            } else {
                String::new()
            }
        }
        HudText::Next { slot, index } => {
            player(slot)
                .next_drops
                .get(index)
                .map_or_else(String::new, |hand| {
                    let [first, second] = hand.colors;
                    format!(
                        "{}{}",
                        BALL_GLYPHS[usize::from(first) % BALL_GLYPHS.len()],
                        BALL_GLYPHS[usize::from(second) % BALL_GLYPHS.len()]
                    )
                })
        }
        HudText::Character(slot) => player(slot).drop_set_id.0.clone(),
        HudText::Scoreline => format!(
            "ROUND {} · BO3 · {} : {}",
            snapshot.round + 1,
            snapshot.wins[0],
            snapshot.wins[1]
        ),
        HudText::Phase => match snapshot.phase {
            game_core::match_state::MatchPhase::RoundIntro { remaining_ticks } => {
                format!("{}", remaining_ticks / 60 + 1)
            }
            game_core::match_state::MatchPhase::Completed(_) => "FINISH".to_owned(),
            _ => String::new(),
        },
    }
}

// ---- construction ----------------------------------------------------------

fn label(commands: &mut Commands, font: &Handle<Font>, size: f32, value: &str) -> Entity {
    commands
        .spawn((
            Text::new(value.to_owned()),
            TextFont {
                font: FontSource::Handle(font.clone()),
                font_size: FontSize::Px(size),
                ..default()
            },
            TextColor(TEXT),
        ))
        .id()
}

fn value_text(commands: &mut Commands, font: &Handle<Font>, size: f32, field: HudText) -> Entity {
    commands
        .spawn((
            field,
            Text::new(String::new()),
            TextFont {
                font: FontSource::Handle(font.clone()),
                font_size: FontSize::Px(size),
                ..default()
            },
            TextColor(TEXT),
        ))
        .id()
}

fn column(commands: &mut Commands, gap: f32) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(gap),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id()
}

fn panel(commands: &mut Commands, width: f32, gap: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: px(width),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(gap),
                padding: UiRect::all(px(10)),
                border_radius: BorderRadius::all(px(10)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .id()
}

fn build_hud(commands: &mut Commands, instance: MatchInstanceId, font: &Handle<Font>) {
    let root = commands
        .spawn((
            HudRoot(instance),
            // The HUD is the bottom layer: pages overlay it explicitly rather
            // than relying on spawn order, which does not decide UI stacking.
            GlobalZIndex(0),
            Node {
                position_type: PositionType::Absolute,
                width: px(1920),
                height: px(1080),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(16),
                padding: UiRect::all(px(24)),
                ..default()
            },
            BackgroundColor(GROUND),
        ))
        .id();

    // Top bar: both characters and the match score, unobstructed above the boards.
    let top = commands
        .spawn((
            Node {
                width: px(1600),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    let p1_name = value_text(commands, font, 30.0, HudText::Character(0));
    let centre = column(commands, 4.0);
    let scoreline = value_text(commands, font, 34.0, HudText::Scoreline);
    let phase = value_text(commands, font, 28.0, HudText::Phase);
    commands.entity(centre).add_children(&[scoreline, phase]);
    let p2_name = value_text(commands, font, 30.0, HudText::Character(1));
    commands
        .entity(top)
        .add_children(&[p1_name, centre, p2_name]);
    commands.entity(root).add_child(top);

    // The boards sit hard against the left and right edges, with the shared
    // columns between them: presentation keeps both boards at the sides so a
    // single gaze covers own board, next hands and opponent pressure.
    let main = commands
        .spawn((
            Node {
                width: px(1840),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(24),
                align_items: AlignItems::FlexStart,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    commands.entity(root).add_child(main);

    let side = |commands: &mut Commands, slot: usize| {
        let side = column(commands, 10.0);
        let queue = queue_panel(commands, font, slot);
        let board = board_grid(commands, font, slot);
        commands.entity(side).add_children(&[queue, board]);
        side
    };
    let middle = |commands: &mut Commands, slot: usize| {
        let middle = column(commands, 14.0);
        let next = next_panel(commands, font, slot);
        let fever = fever_panel(commands, font, slot);
        commands.entity(middle).add_children(&[next, fever]);
        middle
    };

    let p1_side = side(commands, 0);
    let p1_mid = middle(commands, 0);
    let p2_mid = middle(commands, 1);
    let p2_side = side(commands, 1);
    commands
        .entity(main)
        .add_children(&[p1_side, p1_mid, p2_mid, p2_side]);
}

/// Both nuisance queues with exact counts, above the board on the outer side.
fn queue_panel(commands: &mut Commands, font: &Handle<Font>, slot: usize) -> Entity {
    let panel = panel(commands, 300.0, 4.0);
    let heading = label(commands, font, 20.0, "NUISANCE");
    let pending_row = commands
        .spawn((
            Node {
                width: px(260),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    let pending_label = label(commands, font, 20.0, "PENDING");
    let pending = value_text(commands, font, 24.0, HudText::PendingGarbage(slot));
    commands
        .entity(pending_row)
        .add_children(&[pending_label, pending]);

    let fever_row = commands
        .spawn((
            Node {
                width: px(260),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    let fever_label = label(commands, font, 20.0, "FEVER");
    let fever = value_text(commands, font, 24.0, HudText::FeverGarbage(slot));
    commands
        .entity(fever_row)
        .add_children(&[fever_label, fever]);

    let score = value_text(commands, font, 22.0, HudText::Score(slot));
    commands
        .entity(panel)
        .add_children(&[heading, pending_row, fever_row, score]);
    panel
}

/// The next three hands, first one largest.
fn next_panel(commands: &mut Commands, font: &Handle<Font>, slot: usize) -> Entity {
    let panel = panel(commands, 200.0, 8.0);
    let heading = label(commands, font, 20.0, "NEXT");
    commands.entity(panel).add_child(heading);
    for index in 0..3 {
        let size = if index == 0 { 40.0 } else { 28.0 };
        let hand = value_text(commands, font, size, HudText::Next { slot, index });
        commands.entity(panel).add_child(hand);
    }
    let chain = value_text(commands, font, 24.0, HudText::Chain(slot));
    commands.entity(panel).add_child(chain);
    panel
}

/// Gauge, remaining time and puzzle target.
fn fever_panel(commands: &mut Commands, font: &Handle<Font>, slot: usize) -> Entity {
    let panel = panel(commands, 200.0, 6.0);
    let heading = label(commands, font, 20.0, "FEVER");
    let gauge = value_text(commands, font, 26.0, HudText::FeverGauge(slot));
    let time = value_text(commands, font, 24.0, HudText::FeverTime(slot));
    let target = value_text(commands, font, 24.0, HudText::FeverTarget(slot));
    commands
        .entity(panel)
        .add_children(&[heading, gauge, time, target]);
    panel
}

/// A 6x12 grid of reusable cells.
fn board_grid(commands: &mut Commands, font: &Handle<Font>, slot: usize) -> Entity {
    let board = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(CELL_GAP),
                padding: UiRect::all(px(8)),
                border_radius: BorderRadius::all(px(10)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .id();

    for row in 0..VISIBLE_ROWS {
        let line = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(CELL_GAP),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .id();
        for column in 0..BOARD_COLUMNS {
            let cell = commands
                .spawn((
                    BoardCell { slot, column, row },
                    Node {
                        width: px(CELL),
                        height: px(CELL),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    },
                    BorderColor::all(GRID),
                    BackgroundColor(GRID),
                ))
                .id();
            // Every cell owns a glyph node so a ball's non-colour cue is a
            // value write rather than a spawn during play.
            let glyph = commands
                .spawn((
                    Text::new(String::new()),
                    TextFont {
                        font: FontSource::Handle(font.clone()),
                        font_size: FontSize::Px(22.0),
                        ..default()
                    },
                    TextColor(GROUND),
                ))
                .id();
            commands.entity(cell).add_child(glyph);
            commands.entity(line).add_child(cell);
        }
        commands.entity(board).add_child(line);
    }
    board
}
