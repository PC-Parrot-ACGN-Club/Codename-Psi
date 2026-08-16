//! The transient half of the in-match picture: particles, the Fever band and
//! the screen shift an arriving attack causes.
//!
//! Everything here is disposable. A frame that never runs these systems still
//! shows a complete board, because nothing they draw carries information the
//! resident HUD does not already carry.

use bevy::prelude::*;

use crate::hud::{BoardCell, HudRoot};
use crate::match_flow::MatchInstanceId;
use crate::presentation::PresentationEffects;
use crate::settings::UserSettings;
use crate::simulation::{LatestStepReport, RulesSimulation};

use game_core::board::{Cell, Coord};
use game_core::match_state::MatchEvent;
use game_core::resolution::ResolutionPhase;

#[derive(Debug, Default)]
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy::ui::UiPlugin>() {
            return;
        }
        app.init_resource::<EffectState>().add_systems(
            Update,
            (
                forget_released_instance,
                spawn_clear_particles,
                advance_particles,
                drive_fever_band,
                shift_screen,
            )
                .chain(),
        );
    }
}

/// How long a mark lives, in rule ticks.
const MARK_LIFE_TICKS: u64 = 24;
/// How far a mark drifts over its life, in cells.
const MARK_DRIFT_CELLS: f32 = 1.4;
/// Cell pitch, matching the board grid the marks are drawn over.
const CELL_PITCH: f32 = crate::hud::CELL_PITCH;

/// How long the band entering Fever is up, in rule ticks.
const FEVER_BAND_TICKS: u64 = 45;
/// How long a screen shift lasts, in rule ticks.
const SHIFT_TICKS: u64 = 18;
/// Widest screen shift, in virtual-canvas pixels.
const SHIFT_PIXELS: f32 = 14.0;
/// Attack size that shifts the screen as far as it goes.
const SHIFT_FULL_ATTACK: f32 = 30.0;

/// What the transient layer has already reacted to.
#[derive(Debug, Default, Resource)]
struct EffectState {
    instance: Option<MatchInstanceId>,
    /// Chain link each slot last left marks for.
    last_link: [Option<u8>; 2],
    /// Whether each slot was in Fever when last looked at.
    in_fever: [bool; 2],
    /// Tick the current screen shift started on, and how hard it hit.
    shift: Option<(u64, f32)>,
}

impl EffectState {
    fn adopt(&mut self, instance: MatchInstanceId) -> bool {
        if self.instance == Some(instance) {
            return false;
        }
        *self = Self {
            instance: Some(instance),
            ..Self::default()
        };
        true
    }
}

/// One disposable mark left where a ball was cleared.
///
/// Public so a test can count what a clear left and watch it expire; nothing
/// outside this module writes one.
#[derive(Debug, Component)]
pub struct ClearMark {
    born_tick: u64,
    drift: Vec2,
}

/// The band that plays while a participant enters Fever.
#[derive(Debug, Component)]
struct FeverBand {
    born_tick: u64,
}

/// Drop what the previous match left behind.
///
/// The marks themselves are children of the board cells and go with the HUD;
/// what has to be cleared here is the memory of which links and Fever entries
/// were already reacted to.
fn forget_released_instance(
    instance: Option<Res<MatchInstanceId>>,
    mut state: ResMut<EffectState>,
) {
    match instance {
        Some(instance) => {
            state.adopt(*instance);
        }
        None => *state = EffectState::default(),
    }
}

/// Leave marks on the cells a link is about to clear.
///
/// Once per link, on the tick its preview opens: that is the beat the
/// presentation contract puts the hit on, and keying on the link number means a
/// dropped frame costs the marks rather than doubling them.
fn spawn_clear_particles(
    simulation: Option<Res<RulesSimulation>>,
    settings: Res<UserSettings>,
    cells: Query<(Entity, &BoardCell)>,
    marks: Query<(), With<ClearMark>>,
    mut state: ResMut<EffectState>,
    mut commands: Commands,
) {
    let Some(simulation) = simulation else {
        return;
    };
    let effects = PresentationEffects::of(settings.animation_intensity);
    let tick = simulation.0.match_tick();
    let budget = crate::presentation::FeedbackBudget::default().transient_entities;
    let mut live = marks.iter().count();

    for slot in 0..2 {
        let Some(player) = simulation.0.round().player(slot) else {
            continue;
        };
        let Some(ResolutionPhase::ClearPreview { facts, .. }) =
            player.resolution().map(|resolution| resolution.phase())
        else {
            state.last_link[slot] = None;
            continue;
        };
        if state.last_link[slot] == Some(facts.chain_index) {
            continue;
        }
        state.last_link[slot] = Some(facts.chain_index);

        let cleared: Vec<Coord> = facts
            .cleared_colored_coords
            .iter()
            .chain(&facts.cleared_nuisance_coords)
            .copied()
            .collect();
        let per_cell = effects.marks_per_cell();
        for (ordinal, coord) in cleared.iter().enumerate() {
            // At the lowest density only the first cleared cell is marked, so
            // the link still shows one hint and nothing more.
            let count = if per_cell == 0 {
                usize::from(ordinal == 0)
            } else {
                per_cell
            };
            let Some(cell) = cells.iter().find_map(|(entity, cell)| {
                (cell.slot == slot
                    && cell.column == coord.x()
                    && cell.row + player.board().geometry().hidden_rows() == coord.y())
                .then_some(entity)
            }) else {
                continue;
            };
            let color = match player.board().get(*coord) {
                Cell::Color(id) => crate::hud::ball_color(id),
                _ => crate::hud::nuisance_color(),
            };
            for index in 0..count {
                if live >= budget {
                    return;
                }
                live += 1;
                let mark = commands
                    .spawn((
                        ClearMark {
                            born_tick: tick,
                            drift: mark_drift(*coord, index),
                        },
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(18),
                            top: px(18),
                            width: px(8),
                            height: px(8),
                            border_radius: BorderRadius::all(px(4)),
                            ..default()
                        },
                        BackgroundColor(color),
                    ))
                    .id();
                commands.entity(cell).add_child(mark);
            }
        }
    }
}

/// Which way one mark leaves its cell.
///
/// Derived from the cell and the mark's own index rather than drawn from any
/// random source: the rules' streams are theirs alone, and a decorative spread
/// must not be able to move them.
fn mark_drift(coord: Coord, index: usize) -> Vec2 {
    let seed = u32::from(coord.x()) * 7 + u32::from(coord.y()) * 13 + index as u32 * 29;
    #[expect(
        clippy::cast_precision_loss,
        reason = "the seed is a small spread index, not a measurement"
    )]
    let angle = (seed % 16) as f32 / 16.0 * std::f32::consts::TAU;
    // Biased upward, so a clear reads as balls leaving the field.
    Vec2::new(angle.cos(), angle.sin().mul_add(0.5, -0.6))
}

/// Move and fade every live mark, and drop the ones whose life is over.
fn advance_particles(
    simulation: Option<Res<RulesSimulation>>,
    marks: Query<(Entity, &ClearMark, &mut UiTransform, &mut BackgroundColor)>,
    mut commands: Commands,
) {
    let Some(simulation) = simulation else {
        return;
    };
    let tick = simulation.0.match_tick();
    for (entity, mark, mut transform, mut color) in marks {
        let age = tick.saturating_sub(mark.born_tick);
        if age >= MARK_LIFE_TICKS {
            commands.entity(entity).despawn();
            continue;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "a mark lives for a few dozen ticks"
        )]
        let progress = age as f32 / MARK_LIFE_TICKS as f32;
        transform.translation.x = px(mark.drift.x * progress * MARK_DRIFT_CELLS * CELL_PITCH);
        transform.translation.y = px(mark.drift.y * progress * MARK_DRIFT_CELLS * CELL_PITCH);
        transform.scale = Vec2::splat(1.0 - progress * 0.5);
        color.0 = color.0.with_alpha(1.0 - progress);
    }
}

/// Put a band over the screen while a participant enters Fever.
///
/// At full intensity the band sweeps and fades; reduced holds a static frame
/// for the same span, which is the documented substitute for a screen-level
/// effect rather than the absence of one.
fn drive_fever_band(
    simulation: Option<Res<RulesSimulation>>,
    settings: Res<UserSettings>,
    roots: Query<Entity, With<HudRoot>>,
    bands: Query<(Entity, &FeverBand, &mut BorderColor, &mut BackgroundColor)>,
    mut state: ResMut<EffectState>,
    mut commands: Commands,
) {
    let Some(simulation) = simulation else {
        return;
    };
    let effects = PresentationEffects::of(settings.animation_intensity);
    let tick = simulation.0.match_tick();

    for slot in 0..2 {
        let in_fever = simulation
            .0
            .round()
            .player(slot)
            .is_some_and(|player| player.fever().active());
        let entered = in_fever && !state.in_fever[slot];
        state.in_fever[slot] = in_fever;
        if !entered {
            continue;
        }
        let Some(root) = roots.iter().next() else {
            continue;
        };
        let band = commands
            .spawn((
                FeverBand { born_tick: tick },
                Node {
                    position_type: PositionType::Absolute,
                    width: px(1920),
                    height: px(1080),
                    border: UiRect::all(px(10)),
                    ..default()
                },
                BorderColor::all(FEVER_BAND),
                BackgroundColor(Color::NONE),
                GlobalZIndex(5),
            ))
            .id();
        commands.entity(root).add_child(band);
    }

    for (entity, band, mut border, mut fill) in bands {
        let age = tick.saturating_sub(band.born_tick);
        if age >= FEVER_BAND_TICKS {
            commands.entity(entity).despawn();
            continue;
        }
        if !effects.interpolate {
            *border = BorderColor::all(FEVER_BAND);
            fill.0 = Color::NONE;
            continue;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "the band lives for a few dozen ticks"
        )]
        let progress = age as f32 / FEVER_BAND_TICKS as f32;
        *border = BorderColor::all(FEVER_BAND.with_alpha(1.0 - progress));
        fill.0 = FEVER_BAND.with_alpha(0.18 * (1.0 - progress));
    }
}

/// Colour of the band a Fever entry puts over the screen.
const FEVER_BAND: Color = Color::srgb(0.98, 0.72, 0.24);

/// Nudge the whole HUD when an attack lands, then settle it back.
///
/// Reduced intensity keeps the composition fixed instead, so the arrival is
/// still announced by the queue and the text and never by moving the frame.
fn shift_screen(
    simulation: Option<Res<RulesSimulation>>,
    report: Res<LatestStepReport>,
    settings: Res<UserSettings>,
    roots: Query<&mut UiTransform, With<HudRoot>>,
    mut state: ResMut<EffectState>,
) {
    let Some(simulation) = simulation else {
        return;
    };
    let effects = PresentationEffects::of(settings.animation_intensity);
    let tick = simulation.0.match_tick();

    if let Some(report) = report.0.as_ref()
        && report.match_tick == tick
    {
        let landed: u32 = report
            .events
            .iter()
            .filter_map(|event| match event {
                MatchEvent::AttackArbitrated { sent, .. } => Some(*sent),
                _ => None,
            })
            .sum();
        if landed > 0 {
            #[expect(
                clippy::cast_precision_loss,
                reason = "the strength is a display ratio, not an exact count"
            )]
            let strength = (landed as f32 / SHIFT_FULL_ATTACK).clamp(0.2, 1.0);
            state.shift = Some((tick, strength));
        }
    }

    let offset = match state.shift {
        Some((born, strength)) if effects.interpolate => {
            let age = tick.saturating_sub(born);
            if age >= SHIFT_TICKS {
                state.shift = None;
                0.0
            } else {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "the shift lives for a few dozen ticks"
                )]
                let progress = age as f32 / SHIFT_TICKS as f32;
                let decay = 1.0 - progress;
                (progress * std::f32::consts::TAU * 2.0).sin() * decay * strength * SHIFT_PIXELS
            }
        }
        _ => 0.0,
    };

    for mut transform in roots {
        transform.translation.x = px(offset);
    }
}
