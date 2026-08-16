//! The in-match HUD: the resident half of the presentation layer.
//!
//! Everything here is rebuilt from the latest snapshot, never accumulated from
//! events. Entities are spawned once per match instance and then only have
//! their values written, so a high-feedback tick does not churn the ECS.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::text::FontSource;
use game_core::board::{Cell, Coord};

use crate::app_state::AppState;
use crate::i18n::Localization;
use crate::match_flow::MatchInstanceId;
use crate::presentation::{
    FeedbackLines, MatchPresentationSnapshot, NUISANCE_ICON_SLOTS, PresentationEffects,
    build_snapshot, nuisance_icons,
};
use crate::settings::UserSettings;
use crate::simulation::{LatestStepReport, RulesSimulation};
use crate::ui::UiFont;

/// Visible board rows. Hidden rows stay out of the HUD by definition.
const VISIBLE_ROWS: u8 = 12;
const BOARD_COLUMNS: u8 = 6;
const CELL: f32 = 60.0;
const CELL_GAP: f32 = 2.0;
/// Padding inside the board panel, on every side.
const BOARD_PAD: f32 = 8.0;
/// `6 × 60 + 5 × 2 + 2 × 8`. The five layout columns are sized around it.
const BOARD_WIDTH: f32 = 386.0;
/// `12 × 60 + 11 × 2 + 2 × 8`.
const BOARD_HEIGHT: f32 = 758.0;
/// The outer column carrying one portrait and its name.
const SIDE_COLUMN: f32 = 394.0;
/// The channel between the boards: both NEXT columns and both Fever panels.
const CHANNEL_WIDTH: f32 = 360.0;
/// One channel sub-column, one per player.
const CHANNEL_COLUMN: f32 = 152.0;
/// Where the garbage row starts, leaving the round line above it.
const BOARD_TOP: f32 = 96.0;

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

/// A non-colour cue per colour id, drawn only under colour assist.
///
/// Matching is decided by colour alone, so the symbol adds nothing to play and
/// stays off by default. It exists for players who cannot separate the palette
/// by hue -- this palette puts blue and purple at nearly the same lightness,
/// which is exactly the pair the common forms of colour blindness merge.
const BALL_GLYPHS: [&str; 5] = ["●", "▲", "■", "◆", "★"];

const NUISANCE_COLOR: Color = Color::srgb(0.42, 0.44, 0.48);
/// The Fever queue's colour, which is what tells it apart from the normal
/// queue now that both live on one row.
const FEVER_QUEUE: Color = Color::srgb(0.90, 0.78, 0.30);
/// Nuisance keeps its mark unconditionally: it is not one colour among the
/// five but a different kind of ball, and neutral grey alone reads as an empty
/// cell at a glance.
///
/// U+00D7, not one of the heavier multiplication crosses: the shipped font has
/// no glyph for those, and a symbol the font cannot draw is a blank cell.
const NUISANCE_GLYPH: &str = "×";

/// The symbol standing for one queue tier, in `NUISANCE_UNITS` order.
///
/// The set is ordered by visual weight so a heavier queue reads as heavier
/// without the player having to recall which unit each symbol carries, and it
/// shares no symbol with the ball cues above.
const NUISANCE_TIER_GLYPHS: [&str; 7] = [
    "\u{2297}", "\u{25a9}", "\u{25c8}", "\u{2606}", "\u{25c9}", "\u{25ce}", "\u{25cb}",
];

/// The symbol for a queue tier unit.
fn tier_glyph(unit: u32) -> &'static str {
    crate::presentation::NUISANCE_UNITS
        .iter()
        .position(|candidate| *candidate == unit)
        .and_then(|index| NUISANCE_TIER_GLYPHS.get(index).copied())
        .unwrap_or("")
}

/// The glyph a cell shows, given whether the player asked for the extra cue.
fn cell_glyph(occupant: Cell, color_assist: bool) -> &'static str {
    match occupant {
        Cell::Empty => "",
        Cell::Color(id) if color_assist => BALL_GLYPHS[usize::from(id) % BALL_GLYPHS.len()],
        Cell::Color(_) => "",
        Cell::Nuisance => NUISANCE_GLYPH,
    }
}

/// The fill for one occupant.
fn cell_color(occupant: Cell) -> Color {
    match occupant {
        Cell::Empty => GRID,
        Cell::Color(id) => BALL_COLORS[usize::from(id) % BALL_COLORS.len()],
        Cell::Nuisance => NUISANCE_COLOR,
    }
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy::ui::UiPlugin>() {
            return;
        }
        app.init_resource::<MatchFeedback>()
            .add_systems(Update, (spawn_hud, release_hud, refresh_hud).chain())
            .add_systems(Update, refresh_portraits.after(refresh_hud));
    }
}

/// The character presentation each side is drawn with.
///
/// Resolved once per match instance rather than once per frame: resolving is
/// what records the one diagnostic a missing catalog is allowed to produce, and
/// a per-frame resolve would repeat it sixty times a second.
struct MatchPortraits {
    resolved: [Option<crate::character_presentation::ResolvedCharacterPresentation>; 2],
    instance: Option<MatchInstanceId>,
    /// Whether the catalog had been published when this was resolved.
    ///
    /// The read is asynchronous and a match can start before it lands, so a
    /// resolution made without it is provisional: it is the substitute, and it
    /// has to be redone once the real catalog arrives. Without this the first
    /// match of a cold start keeps the substitute for its whole length.
    catalog_published: bool,
}

/// Everything the portraits read.
#[derive(bevy::ecs::system::SystemParam)]
struct PortraitInputs<'w> {
    simulation: Option<Res<'w, RulesSimulation>>,
    report: Res<'w, LatestStepReport>,
    settings: Res<'w, UserSettings>,
    catalog: Option<Res<'w, crate::data::CharacterPresentationData>>,
    rules: Option<Res<'w, crate::data::RulesData>>,
    feedback: Res<'w, MatchFeedback>,
    instance: Option<Res<'w, MatchInstanceId>>,
}

/// Everything the portraits write.
#[derive(bevy::ecs::system::SystemParam)]
struct PortraitEntities<'w, 's> {
    circles: Query<
        'w,
        's,
        (
            &'static Portrait,
            &'static mut BorderColor,
            &'static mut BackgroundColor,
            &'static mut UiTransform,
        ),
        Without<NameBar>,
    >,
    bars: Query<
        'w,
        's,
        (
            &'static NameBar,
            &'static mut BackgroundColor,
            &'static mut BorderColor,
        ),
        Without<Portrait>,
    >,
    badges: Query<
        'w,
        's,
        (
            &'static PortraitBadge,
            &'static mut Text,
            &'static mut TextColor,
        ),
    >,
}

/// What each side is currently being told about the last few ticks.
///
/// One-shot facts do not survive in the snapshot, so the lines they leave are
/// kept here and expire on their own; everything else the HUD draws is read
/// back out of the snapshot every frame.
#[derive(Debug, Default, Resource)]
struct MatchFeedback(FeedbackLines);

/// Root of the HUD, tagged with the instance it belongs to.
#[derive(Debug, Component)]
pub struct HudRoot(MatchInstanceId);

/// One board cell.
///
/// Visible to the transient layer, which hangs its disposable marks off the
/// cell they belong to rather than computing screen positions of its own.
#[derive(Debug, Component)]
pub struct BoardCell {
    pub slot: usize,
    pub column: u8,
    /// Visible row, counted from the top.
    pub row: u8,
}

/// The fill one ball colour is drawn with.
#[must_use]
pub fn ball_color(id: u8) -> Color {
    BALL_COLORS[usize::from(id) % BALL_COLORS.len()]
}

/// The fill nuisance is drawn with.
#[must_use]
pub const fn nuisance_color() -> Color {
    NUISANCE_COLOR
}

/// One icon slot of one queue.
///
/// The slots are spawned once and then only written, so a queue that grows or
/// shrinks changes glyphs rather than the entity tree.
#[derive(Debug, Component)]
struct QueueIcon {
    slot: usize,
    /// Board channel the queue belongs to: normal, then Fever.
    channel: usize,
    index: usize,
}

/// One cell of a Fever gauge, filled in place as the gauge grows.
#[derive(Debug, Component)]
struct FeverSegment {
    slot: usize,
    index: u8,
}

/// The name bar under a portrait, which carries that player's colour.
#[derive(Debug, Component)]
struct NameBar(usize);

/// One participant's portrait circle.
#[derive(Debug, Component)]
struct Portrait(usize);

/// The badge glyph inside a portrait.
#[derive(Debug, Component)]
struct PortraitBadge(usize);

/// Diameter of a portrait circle. It fills its outer column, which is the only
/// area the layout reserves for character presentation.
const PORTRAIT_SIZE: f32 = 320.0;
/// How far a portrait may lift, in virtual-canvas pixels.
const PORTRAIT_LIFT: f32 = 34.0;

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
    Feedback(usize),
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

/// Every HUD entity one refresh writes to.
///
/// Bundled because the board cells, the preview cells and the glyph nodes all
/// reach for `BackgroundColor` or `Text`; the `Without` filters are what prove
/// to the scheduler that the three sets are disjoint.
#[derive(bevy::ecs::system::SystemParam)]
struct HudCells<'w, 's> {
    board: Query<
        'w,
        's,
        (
            &'static BoardCell,
            &'static mut BackgroundColor,
            &'static mut BorderColor,
            &'static mut UiTransform,
            &'static Children,
        ),
        Without<NextCell>,
    >,
    previews: Query<
        'w,
        's,
        (
            &'static NextCell,
            &'static mut BackgroundColor,
            &'static Children,
        ),
        Without<BoardCell>,
    >,
    texts: Query<'w, 's, (&'static HudText, &'static mut Text)>,
    queue_icons: Query<'w, 's, (&'static QueueIcon, &'static mut Text), Without<HudText>>,
    fever_segments: Query<'w, 's, (&'static FeverSegment, &'static mut BackgroundColor), FeverOnly>,
    glyphs: Query<'w, 's, &'static mut Text, GlyphOnly>,
}

/// Gauge cells are the only nodes carrying [`FeverSegment`], so excluding the
/// two cell queries keeps the borrow disjoint from them.
type FeverOnly = (Without<BoardCell>, Without<NextCell>);

/// Text nodes that carry a ball's symbol rather than a HUD value.
///
/// A glyph node is the child of a cell, so it is excluded from both cell
/// queries by the components it does not have.
type GlyphOnly = (
    Without<HudText>,
    Without<BoardCell>,
    Without<NextCell>,
    Without<QueueIcon>,
);

/// Everything one HUD refresh reads.
///
/// Bundled for the same reason as [`PortraitInputs`]: the refresh needs the
/// rules instance, the last report, the settings, the locale and the roster at
/// once, and listing them individually puts the system over the argument
/// budget without making any of them easier to find.
#[derive(bevy::ecs::system::SystemParam)]
struct HudInputs<'w> {
    state: Res<'w, State<AppState>>,
    simulation: Option<Res<'w, RulesSimulation>>,
    report: Res<'w, LatestStepReport>,
    settings: Res<'w, UserSettings>,
    localization: Res<'w, Localization>,
    rules: Option<Res<'w, crate::data::RulesData>>,
}

/// Write the latest snapshot onto the HUD.
fn refresh_hud(inputs: HudInputs, mut feedback: ResMut<MatchFeedback>, mut cells: HudCells) {
    // The pause and settings pages sit over a live board, so the HUD keeps
    // showing the last snapshot rather than blanking while they are up.
    if !matches!(
        inputs.state.get(),
        AppState::Match | AppState::Paused | AppState::Settings
    ) {
        return;
    }
    let Some(simulation) = inputs.simulation.as_ref() else {
        return;
    };
    let settings = &inputs.settings;
    let localization = &inputs.localization;
    let view = simulation.0.view();
    let Some(snapshot) = build_snapshot(
        Some(&view),
        inputs.report.0.as_ref(),
        simulation.0.spec(),
        settings.animation_intensity,
    ) else {
        return;
    };

    if let Some(report) = inputs.report.0.as_ref() {
        feedback.0.observe(report);
    }

    let overlays: [SlotOverlay; 2] =
        std::array::from_fn(|slot| slot_overlay(&snapshot.players[slot], snapshot.effects));

    for (cell, mut background, mut border, mut transform, children) in &mut cells.board {
        let player = &snapshot.players[cell.slot];
        let overlay = &overlays[cell.slot];
        let y = player.board.geometry().hidden_rows() + cell.row;
        let key = (cell.column, y);

        let moving = overlay.moving.get(&key);
        let occupant = if let Some(moving) = moving {
            moving.occupant
        } else if overlay.hidden.contains(&key) {
            Cell::Empty
        } else {
            overlay.active.get(&key).map_or_else(
                || Coord::new(cell.column, y).map_or(Cell::Empty, |coord| player.board.get(coord)),
                |color| Cell::Color(*color),
            )
        };
        let mut color = cell_color(occupant);
        // The preset a Fever puzzle starts with is drawn back, so the chain the
        // player is handed does not read as balls they stacked themselves.
        if occupant.is_occupied() && overlay.preset.contains(&key) {
            color = color.with_alpha(PRESET_ALPHA);
        }
        let glyph = cell_glyph(occupant, settings.color_assist);

        // Reset first: a cell that was posed last frame has to return to rest
        // on its own, since nothing else clears what a finished phase left.
        *transform = UiTransform::IDENTITY;
        if let Some(moving) = moving {
            // The fraction of a cell the ball has travelled is spent here, so
            // it slides between rows instead of jumping a whole cell at a time.
            transform.translation.y = px(moving.offset * (CELL + CELL_GAP));
        } else if overlay.clearing.contains(&key) {
            let pose = overlay.clear_pose;
            color = color
                .mix(&Color::WHITE, pose.flash * 0.8)
                .with_alpha(pose.alpha);
            transform.scale = Vec2::splat(pose.scale);
        }
        background.0 = color;
        // Priority: danger line, then the landing outline, then plain grid.
        *border = BorderColor::all(if cell.row == 0 && player.overflow_risk {
            DANGER
        } else if let Some(color) = overlay.landing.get(&key) {
            BALL_COLORS[usize::from(*color) % BALL_COLORS.len()]
        } else {
            GRID
        });
        for child in children.iter() {
            if let Ok(mut text) = cells.glyphs.get_mut(child)
                && text.0 != glyph
            {
                text.0 = glyph.to_owned();
            }
        }
    }

    for (cell, mut background, children) in &mut cells.previews {
        let occupant = snapshot.players[cell.slot]
            .next_drops
            .get(cell.index)
            .and_then(|hand| preview_occupant(hand, cell.dx, cell.dy));

        // No ball at this offset: the cell disappears rather than showing an
        // empty slot, which is what makes the shape readable.
        background.0 = occupant.map_or(Color::NONE, cell_color);
        let glyph = occupant.map_or("", |occupant| cell_glyph(occupant, settings.color_assist));
        for child in children.iter() {
            if let Ok(mut text) = cells.glyphs.get_mut(child)
                && text.0 != glyph
            {
                text.0 = glyph.to_owned();
            }
        }
    }

    let icons: [[Vec<u32>; 2]; 2] = std::array::from_fn(|slot| {
        let player = &snapshot.players[slot];
        [
            nuisance_icons(player.pending_garbage),
            nuisance_icons(player.fever_garbage),
        ]
    });
    for (segment, mut fill) in &mut cells.fever_segments {
        let filled = segment.index < snapshot.players[segment.slot].fever_gauge;
        fill.0 = if filled { FEVER_QUEUE } else { GRID };
    }

    for (icon, mut text) in &mut cells.queue_icons {
        let glyph = icons[icon.slot][icon.channel]
            .get(icon.index)
            .map_or("", |unit| tier_glyph(*unit));
        if text.0 != glyph {
            text.0 = glyph.to_owned();
        }
    }

    // Resolved once per refresh rather than once per text node: both portraits
    // ask for a name, and the roster lookup is the same walk either time.
    let names: [String; 2] = std::array::from_fn(|slot| {
        simulation
            .0
            .spec()
            .characters
            .get(slot)
            .map_or_else(String::new, |id| {
                character_name(inputs.rules.as_deref(), id, localization)
            })
    });

    for (field, mut text) in &mut cells.texts {
        let value = hud_value(field, &snapshot, &feedback.0, localization, &names);
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// Pose, colour and lean for both portraits, and the track marker with them.
///
/// Split from the main refresh because it needs the character catalog and the
/// two portrait entities, and because a portrait is the one part of the HUD
/// that is allowed to be missing: with no catalog the resolver hands out a
/// substitute rather than leaving the circle blank.
fn refresh_portraits(
    inputs: PortraitInputs,
    mut portraits: Local<Option<MatchPortraits>>,
    mut entities: PortraitEntities,
) {
    use crate::character_presentation::{
        CharacterPresentationResolver, ResolvedCharacterPresentation,
    };

    let Some(simulation) = inputs.simulation.as_ref() else {
        return;
    };
    let Some(instance) = inputs.instance.as_ref() else {
        return;
    };
    let instance = **instance;
    let view = simulation.0.view();
    let Some(snapshot) = build_snapshot(
        Some(&view),
        inputs.report.0.as_ref(),
        simulation.0.spec(),
        inputs.settings.animation_intensity,
    ) else {
        return;
    };

    // A new instance may have new characters, so the resolution is redone once
    // per instance rather than once per frame -- and once more if the catalog
    // was still in flight when the instance started.
    let catalog_published = inputs.catalog.is_some();
    if portraits.as_ref().is_none_or(|held| {
        held.instance != Some(instance) || (!held.catalog_published && catalog_published)
    }) {
        let resolution = inputs.catalog.as_ref().map_or(
            crate::data::DataResolution::Failed(crate::data::DataLoadError {
                path: crate::data::PRESENTATION_CHARACTERS_PATH.into(),
                category: crate::data::DataCategory::Presentation,
                cause: crate::data::DataErrorCause::Io("catalog not read yet".into()),
            }),
            |data| data.0.clone(),
        );
        let mut resolver = CharacterPresentationResolver::new(resolution);
        let roster = inputs.rules.as_ref().and_then(|rules| rules.rules());
        let resolved: [Option<ResolvedCharacterPresentation>; 2] = std::array::from_fn(|slot| {
            let id = simulation.0.spec().characters.get(slot)?.clone();
            // The roster is where a character's display name lives, and the
            // substitute badge is cut from it. Without a roster the id stands
            // in, which is still stable and still tells the two sides apart.
            let identity = roster
                .and_then(|library| {
                    library
                        .roster()
                        .characters
                        .iter()
                        .find(|identity| identity.id == id)
                        .cloned()
                })
                .unwrap_or_else(|| game_core::config::CharacterIdentity {
                    display_name_key: id.0.clone(),
                    id: id.clone(),
                });
            Some(resolver.resolve(&identity, slot))
        });
        for diagnostic in resolver.diagnostics() {
            warn!(
                "character {} drawn with a substitute: {}",
                diagnostic.character_id.0, diagnostic.reason
            );
        }
        *portraits = Some(MatchPortraits {
            resolved,
            instance: Some(instance),
            catalog_published,
        });
    }
    let Some(held) = portraits.as_ref() else {
        return;
    };

    for (portrait, mut border, mut fill, mut transform) in &mut entities.circles {
        let Some(resolved) = held.resolved[portrait.0].as_ref() else {
            continue;
        };
        let pose_kind =
            crate::presentation::portrait_pose(&snapshot, &inputs.feedback.0, portrait.0);
        let pose = resolved
            .data
            .poses
            .get(&pose_kind)
            .copied()
            .unwrap_or_default();

        *border = BorderColor::all(rgb(resolved.data.primary_color));
        fill.0 = rgb(resolved.data.secondary_color);
        // Positive offsets lift the portrait. The two portraits sit at
        // opposite edges of the screen, so leaning them toward the middle
        // reads as drift rather than impact; the vertical axis is the one a
        // one-shot jump will use.
        let lift = f32::from(pose.offset) / 24.0 * PORTRAIT_LIFT;
        *transform = UiTransform::IDENTITY;
        transform.translation.y = px(-lift);
        transform.scale = Vec2::splat(f32::from(pose.scale) / 100.0);
    }

    for (bar, mut fill, mut border) in &mut entities.bars {
        let Some(resolved) = held.resolved[bar.0].as_ref() else {
            continue;
        };
        fill.0 = rgb(resolved.data.secondary_color);
        *border = BorderColor::all(rgb(resolved.data.primary_color));
    }

    for (badge, mut text, mut color) in &mut entities.badges {
        let Some(resolved) = held.resolved[badge.0].as_ref() else {
            continue;
        };
        if text.0 != resolved.data.badge.glyph {
            text.0.clone_from(&resolved.data.badge.glyph);
        }
        color.0 = rgb(resolved.data.primary_color);
    }
}

/// The colour one presentation entry names.
fn rgb(color: crate::character_presentation::RgbColor) -> Color {
    Color::srgb_u8(color.r, color.g, color.b)
}

/// What a hand puts at one offset from its pivot, if anything.
fn preview_occupant(hand: &game_core::drop_stream::PendingHand, dx: i8, dy: i8) -> Option<Cell> {
    use game_core::config::ColorSlot;

    hand.template
        .balls()
        .into_iter()
        .find(|ball| ball.dx == dx && ball.dy == dy)
        .map(|ball| {
            let color = match ball.color_slot {
                ColorSlot::First => hand.colors[0],
                ColorSlot::Second => hand.colors[1],
            };
            Cell::Color(color)
        })
}

/// The active group's cells and where it would come to rest.
///
/// Computed once per slot per frame rather than once per cell, and kept out of
/// the rules entirely: the landing outline is an operating aid, so it never
/// reaches the rule state, the checksum or a screenshot baseline.
#[derive(Debug, Default)]
struct SlotOverlay {
    active: HashMap<(u8, u8), u8>,
    landing: HashMap<(u8, u8), u8>,
    /// Balls in flight during `Gravity`, keyed by the cell they are drawn in.
    moving: HashMap<(u8, u8), MovingBall>,
    /// Cells the resolver has already accounted for elsewhere this phase.
    hidden: HashSet<(u8, u8)>,
    /// Cells being previewed for a clear.
    clearing: HashSet<(u8, u8)>,
    /// Cells the loaded Fever puzzle preset put on the board.
    preset: HashSet<(u8, u8)>,
    /// The pose those cells are drawn with this frame.
    clear_pose: ClearPose,
}

/// A ball partway between two cells.
#[derive(Debug, Clone, Copy)]
struct MovingBall {
    occupant: Cell,
    /// How far past the drawn cell the ball has travelled, in cells.
    ///
    /// A grid can only place a ball on whole cells, so the fraction is carried
    /// here and spent as a render offset. Without it a ball would jump a whole
    /// cell at a time and the phase would read as a series of teleports.
    offset: f32,
}

/// How a ball being cleared is drawn at one instant.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ClearPose {
    scale: f32,
    /// How far the fill is mixed toward white.
    flash: f32,
    alpha: f32,
}

impl Default for ClearPose {
    fn default() -> Self {
        Self {
            scale: 1.0,
            flash: 0.0,
            alpha: 1.0,
        }
    }
}

/// How opaque a puzzle's preset ball is drawn.
const PRESET_ALPHA: f32 = 0.55;

/// Fraction of the preview spent on the hit before the ball starts leaving.
const CLEAR_HIT_SHARE: f32 = 0.35;
/// How much of its size a cleared ball keeps at the end of the preview.
const CLEAR_END_SCALE: f32 = 0.35;

impl ClearPose {
    /// The pose for one point of the clear preview.
    ///
    /// Full intensity plays the whole beat the presentation contract asks for:
    /// the hit lands and flashes, then the ball shrinks and fades so it is gone
    /// by the moment the rules commit the clear. Reduced holds a single steady
    /// highlight instead -- the fact is still shown, just not animated.
    fn of(progress: f32, interpolate: bool) -> Self {
        if !interpolate {
            return Self {
                scale: 1.0,
                flash: 1.0,
                alpha: 1.0,
            };
        }
        let progress = progress.clamp(0.0, 1.0);
        let flash = (progress / CLEAR_HIT_SHARE).min(1.0);
        let tail = ((progress - CLEAR_HIT_SHARE) / (1.0 - CLEAR_HIT_SHARE)).clamp(0.0, 1.0);
        Self {
            scale: 1.0 - (1.0 - CLEAR_END_SCALE) * tail,
            flash,
            alpha: 1.0 - tail,
        }
    }
}

/// Place the resolving phases' balls for this frame.
///
/// The rules own the timing: this only reads `elapsed / duration` and poses the
/// balls accordingly. It never reports completion back, so no animation here
/// can hold up a tick.
fn resolve_overlay(
    overlay: &mut SlotOverlay,
    player: &crate::presentation::PlayerPresentationSnapshot,
    effects: PresentationEffects,
) {
    use game_core::view::ResolutionStage;

    let Some(resolution) = player.resolution.as_ref() else {
        return;
    };
    let progress = if resolution.duration_ticks == 0 {
        1.0
    } else {
        f32::from(resolution.elapsed_ticks) / f32::from(resolution.duration_ticks)
    };

    match resolution.stage {
        ResolutionStage::ClearPreview => {
            overlay.clear_pose = ClearPose::of(progress, effects.interpolate);
            for coord in &resolution.clear_cells {
                overlay.clearing.insert((coord.x(), coord.y()));
            }
        }
        ResolutionStage::Gravity => {
            // The board still holds the pre-gravity arrangement, so each moving
            // ball is hidden at its source and drawn at its current position.
            for step in &resolution.gravity_moves {
                let (row, offset) =
                    fall_position(step.from.y(), step.to.y(), progress, effects.interpolate);
                let occupant = Coord::new(step.from.x(), step.from.y())
                    .map_or(Cell::Empty, |coord| player.board.get(coord));
                overlay.hidden.insert((step.from.x(), step.from.y()));
                overlay
                    .moving
                    .insert((step.from.x(), row), MovingBall { occupant, offset });
            }
        }
        _ => {}
    }
}

/// Where a falling ball is partway through the gravity phase.
///
/// Returns the cell it is drawn in and how far past that cell it has travelled,
/// so the caller can spend the fraction as a render offset. Reduced intensity
/// holds the ball at its source and snaps to the target when the phase ends,
/// which is the documented substitute for interpolating.
fn fall_position(from: u8, to: u8, progress: f32, interpolate: bool) -> (u8, f32) {
    let progress = progress.clamp(0.0, 1.0);
    if !interpolate {
        return (if progress >= 1.0 { to } else { from }, 0.0);
    }
    let span = f32::from(to) - f32::from(from);
    let exact = f32::from(from) + span * progress;
    let row = exact.floor();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the interpolant stays between two valid board rows"
    )]
    let cell = row as u8;
    (cell, exact - row)
}

fn slot_overlay(
    player: &crate::presentation::PlayerPresentationSnapshot,
    effects: PresentationEffects,
) -> SlotOverlay {
    let mut overlay = SlotOverlay::default();
    resolve_overlay(&mut overlay, player, effects);
    for coord in &player.preset_cells {
        overlay.preset.insert((coord.x(), coord.y()));
    }
    let Some(group) = player.active_drop.as_ref() else {
        return overlay;
    };
    if let Some(cells) = group.cells(&player.board) {
        for (coord, color) in cells {
            overlay.active.insert((coord.x(), coord.y()), color);
        }
    }
    let mut ghost = *group;
    while ghost.try_translate(&player.board, 0, 1) {}
    if let Some(cells) = ghost.cells(&player.board) {
        for (coord, color) in cells {
            let key = (coord.x(), coord.y());
            if !overlay.active.contains_key(&key) {
                overlay.landing.insert(key, color);
            }
        }
    }
    overlay
}

/// The name shown under a portrait.
///
/// The roster owns the display name key and the catalog owns the text, so the
/// two are joined here rather than at portrait resolution: resolution happens
/// once per match, and a name resolved there would keep the language it was
/// resolved in after the player switched locales.
///
/// Without a roster the id stands in directly instead of being passed to the
/// catalog, so a missing roster does not also register a missing key.
fn character_name(
    rules: Option<&crate::data::RulesData>,
    id: &game_core::config::CharacterId,
    localization: &Localization,
) -> String {
    rules
        .and_then(crate::data::RulesData::rules)
        .and_then(|library| {
            library
                .roster()
                .characters
                .iter()
                .find(|identity| identity.id == *id)
        })
        .map_or_else(
            || id.0.clone(),
            |identity| localization.text(&identity.display_name_key),
        )
}

fn hud_value(
    field: &HudText,
    snapshot: &MatchPresentationSnapshot,
    feedback: &FeedbackLines,
    localization: &Localization,
    names: &[String; 2],
) -> String {
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
        HudText::Feedback(slot) => feedback
            .line(slot, snapshot.match_tick)
            .map_or_else(String::new, |line| line.text(localization)),
        HudText::Character(slot) => names.get(slot).cloned().unwrap_or_default(),
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
    // Five columns: portrait, board, channel, board, portrait. The boards are
    // the players' main gaze area, so they sit against the channel in the
    // middle of the screen rather than against the screen edges.
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
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            BackgroundColor(GROUND),
        ))
        .id();

    let p1_portrait = portrait_column(commands, font, 0);
    let p1_board = board_column(commands, font, 0);
    let channel = channel_column(commands, font);
    let p2_board = board_column(commands, font, 1);
    let p2_portrait = portrait_column(commands, font, 1);
    commands
        .entity(root)
        .add_children(&[p1_portrait, p1_board, channel, p2_board, p2_portrait]);

    // Last child, so it draws over every column it covers.
    let big_word = big_word_layer(commands, font);
    commands.entity(root).add_child(big_word);
}

/// The full-screen word layer: the countdown, and the final `FINISH`.
///
/// The resident layout keeps no room for these. They are states of a layer of
/// their own, so nothing else has to make space for something that is only on
/// screen for a moment.
fn big_word_layer(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let layer = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: px(1920),
                height: px(1080),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    let word = value_text(commands, font, 132.0, HudText::Phase);
    commands.entity(layer).add_child(word);
    layer
}

/// One player's column: the garbage row, the board, and the score under it.
fn board_column(commands: &mut Commands, font: &Handle<Font>, slot: usize) -> Entity {
    let column = commands
        .spawn((
            Node {
                width: px(BOARD_WIDTH),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(px(BOARD_TOP)),
                row_gap: px(10),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();

    let garbage = garbage_row(commands, font, slot);

    // The board and the text over it share one box, so the chain count and the
    // attack figure land inside the area the player is already watching.
    let stack = commands
        .spawn((
            Node {
                width: px(BOARD_WIDTH),
                height: px(BOARD_HEIGHT),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    let board = board_grid(commands, font, slot);
    let overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(18),
                top: px(22),
                width: px(BOARD_WIDTH - 36.0),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    let chain = value_text(commands, font, 46.0, HudText::Chain(slot));
    let feedback = value_text(commands, font, 26.0, HudText::Feedback(slot));
    commands.entity(overlay).add_children(&[chain, feedback]);
    commands.entity(stack).add_children(&[board, overlay]);

    let score = value_text(commands, font, 58.0, HudText::Score(slot));
    commands
        .entity(column)
        .add_children(&[garbage, stack, score]);
    column
}

/// Both queues on one row above the board: tiered icons and exact counts.
///
/// One row is all the space this earns. The Fever queue shares it and is told
/// apart by colour rather than by a heading of its own.
fn garbage_row(commands: &mut Commands, font: &Handle<Font>, slot: usize) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: px(BOARD_WIDTH),
                height: px(64),
                flex_direction: if slot == 0 {
                    FlexDirection::Row
                } else {
                    FlexDirection::RowReverse
                },
                align_items: AlignItems::Center,
                column_gap: px(12),
                padding: UiRect::horizontal(px(6)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();

    let pending_icons = icon_strip(commands, font, slot, 0, TEXT);
    let pending = value_text(commands, font, 34.0, HudText::PendingGarbage(slot));
    let fever_icons = icon_strip(commands, font, slot, 1, FEVER_QUEUE);
    let fever = value_text(commands, font, 26.0, HudText::FeverGarbage(slot));
    commands.entity(fever).insert(TextColor(FEVER_QUEUE));
    commands
        .entity(row)
        .add_children(&[pending_icons, pending, fever_icons, fever]);
    row
}

/// One queue's tiered icons, written in place as the queue changes.
fn icon_strip(
    commands: &mut Commands,
    font: &Handle<Font>,
    slot: usize,
    channel: usize,
    colour: Color,
) -> Entity {
    let icons = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(4),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    for index in 0..NUISANCE_ICON_SLOTS {
        let icon = commands
            .spawn((
                QueueIcon {
                    slot,
                    channel,
                    index,
                },
                Text::new(String::new()),
                TextFont {
                    font: FontSource::Handle(font.clone()),
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(colour),
            ))
            .id();
        commands.entity(icons).add_child(icon);
    }
    icons
}

/// The channel between the boards: the round line, both NEXT columns and both
/// Fever panels.
fn channel_column(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let channel = commands
        .spawn((
            Node {
                width: px(CHANNEL_WIDTH),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(px(26)),
                row_gap: px(30),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();

    let scoreline = value_text(commands, font, 24.0, HudText::Scoreline);

    let pair = |commands: &mut Commands,
                build: fn(&mut Commands, &Handle<Font>, usize) -> Entity| {
        let row = commands
            .spawn((
                Node {
                    width: px(CHANNEL_WIDTH),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::FlexStart,
                    padding: UiRect::horizontal(px(16)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .id();
        let left = build(commands, font, 0);
        let right = build(commands, font, 1);
        commands.entity(row).add_children(&[left, right]);
        row
    };

    let nexts = pair(commands, next_panel);
    let fevers = pair(commands, fever_panel);
    commands
        .entity(channel)
        .add_children(&[scoreline, nexts, fevers]);
    channel
}

/// One participant's portrait column, with the name under the circle.
///
/// The circle carries the character's own colours and badge. Its pose drives a
/// vertical offset and a scale; the portrait stays in its own column, since the
/// two of them are a screen apart and nothing they do reads as a collision.
fn portrait_column(commands: &mut Commands, font: &Handle<Font>, slot: usize) -> Entity {
    let column = commands
        .spawn((
            Node {
                width: px(SIDE_COLUMN),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(px(400)),
                row_gap: px(22),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    let circle = commands
        .spawn((
            Portrait(slot),
            Node {
                width: px(PORTRAIT_SIZE),
                height: px(PORTRAIT_SIZE),
                border: UiRect::all(px(5)),
                border_radius: BorderRadius::all(px(PORTRAIT_SIZE / 2.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor::all(TEXT),
            BackgroundColor(PANEL),
        ))
        .id();
    let badge = commands
        .spawn((
            PortraitBadge(slot),
            Text::new(String::new()),
            TextFont {
                font: FontSource::Handle(font.clone()),
                font_size: FontSize::Px(96.0),
                ..default()
            },
            TextColor(TEXT),
        ))
        .id();
    commands.entity(circle).add_child(badge);
    let bar = commands
        .spawn((
            NameBar(slot),
            Node {
                width: px(PORTRAIT_SIZE * 0.92),
                height: px(52),
                border: UiRect::all(px(3)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BorderColor::all(TEXT),
            BackgroundColor(PANEL),
        ))
        .id();
    let name = value_text(commands, font, 30.0, HudText::Character(slot));
    commands.entity(bar).add_child(name);
    commands.entity(column).add_children(&[circle, bar]);
    column
}

/// The next three hands, first one largest.
fn next_panel(commands: &mut Commands, font: &Handle<Font>, slot: usize) -> Entity {
    let panel = panel(commands, CHANNEL_COLUMN, 8.0);
    let heading = label(commands, font, 18.0, "NEXT");
    commands.entity(panel).add_child(heading);
    for index in 0..3 {
        let size = if index == 0 { 30.0 } else { 21.0 };
        let hand = next_preview(commands, font, slot, index, size);
        commands.entity(panel).add_child(hand);
    }
    panel
}

/// Columns and rows a preview grid needs to hold any hand shape.
///
/// `DropTemplate::balls` places every ball at `dx` in `-1..=1` and `dy` in
/// `-1..=0` around the pivot, so this box covers `I`, `L`, `J` and both `O`
/// layouts without the grid having to know which one it is showing.
const PREVIEW_COLUMNS: i8 = 3;
const PREVIEW_ROWS: i8 = 2;

/// One cell of one hand's preview, addressed by its offset from the pivot.
#[derive(Debug, Component)]
struct NextCell {
    slot: usize,
    index: usize,
    dx: i8,
    dy: i8,
}

/// A preview grid for one upcoming hand.
///
/// Built from the same coloured cells as the board rather than from text: the
/// preview has to be comparable with what will land, and a glyph string can
/// carry neither the hand's colours nor its shape.
fn next_preview(
    commands: &mut Commands,
    font: &Handle<Font>,
    slot: usize,
    index: usize,
    size: f32,
) -> Entity {
    let grid = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(CELL_GAP),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();

    for dy in -PREVIEW_ROWS + 1..=0 {
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
        for dx in -(PREVIEW_COLUMNS / 2)..=PREVIEW_COLUMNS / 2 {
            let cell = commands
                .spawn((
                    NextCell {
                        slot,
                        index,
                        dx,
                        dy,
                    },
                    Node {
                        width: px(size),
                        height: px(size),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(px(size / 7.0)),
                        ..default()
                    },
                    // An offset no ball occupies stays fully transparent, so the
                    // grid shows the hand's silhouette instead of a 3x2 block.
                    BackgroundColor(Color::NONE),
                ))
                .id();
            let glyph = commands
                .spawn((
                    Text::new(String::new()),
                    TextFont {
                        font: FontSource::Handle(font.clone()),
                        font_size: FontSize::Px(size * 0.6),
                        ..default()
                    },
                    TextColor(GROUND),
                ))
                .id();
            commands.entity(cell).add_child(glyph);
            commands.entity(line).add_child(cell);
        }
        commands.entity(grid).add_child(line);
    }
    grid
}

/// Gauge, remaining time and puzzle target.
fn fever_panel(commands: &mut Commands, font: &Handle<Font>, slot: usize) -> Entity {
    let panel = panel(commands, CHANNEL_COLUMN, 6.0);
    let heading = label(commands, font, 18.0, "FEVER");
    let gauge = fever_gauge(commands, slot);
    let counted = value_text(commands, font, 20.0, HudText::FeverGauge(slot));
    let time = value_text(commands, font, 24.0, HudText::FeverTime(slot));
    let target = value_text(commands, font, 24.0, HudText::FeverTarget(slot));
    commands
        .entity(panel)
        .add_children(&[heading, gauge, counted, time, target]);
    panel
}

/// The seven-cell gauge as one horizontal strip: `7 × 18 + 6 × 4` fits the
/// channel column exactly, which is what lets the channel hold both panels.
fn fever_gauge(commands: &mut Commands, slot: usize) -> Entity {
    let strip = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(4),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    for index in 0..7 {
        let cell = commands
            .spawn((
                FeverSegment { slot, index },
                Node {
                    width: px(18),
                    height: px(28),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(GRID),
            ))
            .id();
        commands.entity(strip).add_child(cell);
    }
    strip
}

/// A 6x12 grid of reusable cells.
fn board_grid(commands: &mut Commands, font: &Handle<Font>, slot: usize) -> Entity {
    let board = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(CELL_GAP),
                padding: UiRect::all(px(BOARD_PAD)),
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
                    // Posed by `refresh_hud` during the resolve phases. Present
                    // from the start so a phase never spawns components mid-play.
                    UiTransform::IDENTITY,
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

#[cfg(test)]
mod tests {
    use game_core::config::{DropShape, DropTemplate};
    use game_core::drop_stream::PendingHand;

    use super::{CLEAR_HIT_SHARE, Cell, ClearPose, cell_glyph, fall_position, preview_occupant};

    fn hand(shape: DropShape, colors: [u8; 2]) -> PendingHand {
        PendingHand {
            template: DropTemplate {
                shape,
                vertical_pair_first: Some(true),
            },
            colors,
            turn_id: 0,
        }
    }

    // integration-system/presentation-runtime::TC-011
    #[test]
    fn colour_is_the_only_ball_cue_until_colour_assist_is_on() {
        // Clearing is decided by colour, so the symbol is off by default and
        // every colour reads as a plain fill.
        for id in 0..5 {
            assert_eq!(cell_glyph(Cell::Color(id), false), "");
            assert!(!cell_glyph(Cell::Color(id), true).is_empty());
        }

        // Distinct symbols, or the assist would not separate the colours it
        // exists to separate.
        let assisted: Vec<&str> = (0..5).map(|id| cell_glyph(Cell::Color(id), true)).collect();
        for (index, glyph) in assisted.iter().enumerate() {
            assert!(
                !assisted[index + 1..].contains(glyph),
                "colour {index} shares its symbol with a later colour"
            );
        }

        // Nuisance is a different kind of ball rather than a sixth colour, so
        // its mark does not depend on the setting.
        assert_eq!(
            cell_glyph(Cell::Nuisance, false),
            cell_glyph(Cell::Nuisance, true)
        );
        assert!(!cell_glyph(Cell::Nuisance, false).is_empty());
        assert_eq!(cell_glyph(Cell::Empty, true), "");
    }

    // integration-system/presentation-runtime::TC-011
    #[test]
    fn the_next_preview_carries_both_the_hand_shape_and_its_colours() {
        // `I` occupies the pivot column only, so the preview shows a vertical
        // pair and the side offsets stay empty.
        let i = hand(DropShape::I, [1, 2]);
        assert_eq!(preview_occupant(&i, 0, 0), Some(Cell::Color(1)));
        assert_eq!(preview_occupant(&i, 0, -1), Some(Cell::Color(2)));
        assert_eq!(preview_occupant(&i, 1, 0), None);
        assert_eq!(preview_occupant(&i, -1, 0), None);

        // `L` and `J` differ only in which side the arm is on -- exactly the
        // distinction a two-glyph string could not express.
        let l = hand(DropShape::L, [1, 2]);
        let j = hand(DropShape::J, [1, 2]);
        assert_eq!(preview_occupant(&l, 1, 0), Some(Cell::Color(2)));
        assert_eq!(preview_occupant(&l, -1, 0), None);
        assert_eq!(preview_occupant(&j, -1, 0), Some(Cell::Color(2)));
        assert_eq!(preview_occupant(&j, 1, 0), None);

        // `O` fills a 2x2 block, and the dual layout puts each drawn colour on
        // its own row.
        let o = hand(DropShape::ODual, [3, 4]);
        for dx in [0, 1] {
            assert_eq!(preview_occupant(&o, dx, 0), Some(Cell::Color(3)));
            assert_eq!(preview_occupant(&o, dx, -1), Some(Cell::Color(4)));
        }
        assert_eq!(preview_occupant(&o, -1, 0), None);

        // A single-colour hand draws one colour everywhere, so the second drawn
        // colour must not leak into the preview.
        let mono = hand(DropShape::OMono, [3, 4]);
        for dx in [0, 1] {
            for dy in [0, -1] {
                assert_eq!(preview_occupant(&mono, dx, dy), Some(Cell::Color(3)));
            }
        }
    }

    // integration-system/presentation-runtime::TC-012
    #[test]
    fn a_falling_ball_slides_between_its_ends_without_overshooting() {
        // Endpoints are exact and land on a whole cell, so the ball starts where
        // the rules put it and finishes exactly where the committed board draws it.
        assert_eq!(fall_position(2, 8, 0.0, true), (2, 0.0));
        assert_eq!(fall_position(2, 8, 1.0, true), (8, 0.0));

        // A distance whose midpoint falls between two rows is drawn between
        // them: that offset is the whole difference from snapping cell to cell.
        let (row, offset) = fall_position(2, 7, 0.5, true);
        assert_eq!(row, 4);
        assert!(
            (offset - 0.5).abs() < f32::EPSILON,
            "expected half a cell, got {offset}"
        );

        // Motion is monotonic, so a ball never appears to travel back up.
        let mut previous = (0, 0.0);
        for step in 0..=10 {
            #[expect(clippy::cast_precision_loss, reason = "ten steps")]
            let position = fall_position(0, 10, step as f32 / 10.0, true);
            assert!(
                position >= previous,
                "position went backwards at step {step}"
            );
            previous = position;
        }

        // Out-of-range progress is clamped rather than running off the board.
        assert_eq!(fall_position(3, 5, 2.0, true), (5, 0.0));

        // Reduced intensity holds the start and snaps at the end: the two
        // endpoints are shown and nothing between them.
        assert_eq!(fall_position(2, 8, 0.5, false), (2, 0.0));
        assert_eq!(fall_position(2, 8, 0.99, false), (2, 0.0));
        assert_eq!(fall_position(2, 8, 1.0, false), (8, 0.0));
    }

    // integration-system/presentation-runtime::TC-012
    #[test]
    fn a_cleared_ball_flashes_then_leaves_before_the_rules_remove_it() {
        // The hit lands at full size, so the player sees which balls were hit
        // before anything starts moving.
        let start = ClearPose::of(0.0, true);
        assert_eq!(start.scale, 1.0);
        assert_eq!(start.alpha, 1.0);
        assert_eq!(start.flash, 0.0);

        // The flash comes up during the hit and is complete by the time the
        // ball starts to go.
        assert_eq!(ClearPose::of(CLEAR_HIT_SHARE, true).flash, 1.0);
        assert_eq!(ClearPose::of(CLEAR_HIT_SHARE, true).scale, 1.0);

        // By the end the ball has shrunk and faded out, so the commit that
        // removes it has nothing left to pop away.
        let end = ClearPose::of(1.0, true);
        assert_eq!(end.alpha, 0.0);
        assert!(end.scale < 1.0);

        // Shrinking and fading are monotonic across the phase.
        let mut previous = ClearPose::of(0.0, true);
        for step in 1..=10 {
            #[expect(clippy::cast_precision_loss, reason = "ten steps")]
            let pose = ClearPose::of(step as f32 / 10.0, true);
            assert!(pose.scale <= previous.scale, "scale grew at step {step}");
            assert!(pose.alpha <= previous.alpha, "alpha rose at step {step}");
            previous = pose;
        }

        // Reduced keeps one steady highlight for the whole phase: the fact is
        // still reported, it just is not animated.
        let steady = ClearPose {
            scale: 1.0,
            flash: 1.0,
            alpha: 1.0,
        };
        for step in 0..=10 {
            #[expect(clippy::cast_precision_loss, reason = "ten steps")]
            let pose = ClearPose::of(step as f32 / 10.0, false);
            assert_eq!(pose, steady);
        }
    }
}
