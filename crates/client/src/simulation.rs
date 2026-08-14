//! Ordered 60 Hz fixed-update bridge into the rules layer.

use std::time::Duration;

use bevy::prelude::*;
use game_core::input::{PlayerActions, TickInputs};

use game_core::MatchState;

use crate::app_state::AppState;
use crate::input::LocalInputSampler;

pub const FIXED_HZ: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum FixedGameSet {
    Input,
    Rules,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedStage {
    Input,
    Rules,
}

/// Canonical inputs formed by `FixedGameSet::Input` for the current fixed tick.
///
/// The `consumed` flag keeps the rules stage from advancing twice on the same
/// tick inputs and makes an unconsumed tick observable.
#[derive(Debug, Default, Resource)]
pub struct CurrentTickInputs {
    pub inputs: TickInputs,
    consumed: bool,
}

impl CurrentTickInputs {
    #[must_use]
    pub const fn is_consumed(&self) -> bool {
        self.consumed
    }
}

/// The running match, present only once a specification has been frozen.
///
/// `client::simulation` is a thin bridge: it forms the tick's canonical inputs
/// and calls `MatchState::step`. It holds no rule judgement of its own.
#[derive(Debug, Resource)]
pub struct RulesSimulation(pub MatchState);

/// Rule facts advanced only by `FixedGameSet::Rules`.
///
/// When a match is running these mirror `MatchState`; with no match frozen yet
/// the checksum folds the consumed inputs instead, which keeps the fixed
/// schedule observable before a specification exists.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RuleState {
    pub tick: u64,
    pub checksum: u64,
}

impl RuleState {
    /// FNV-1a fold over the tick's participant inputs.
    fn advance(&mut self, inputs: &TickInputs) {
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut checksum = self.checksum ^ 0xcbf2_9ce4_8422_2325;
        checksum = (checksum ^ inputs.len() as u64).wrapping_mul(PRIME);
        // Folds the crate's stable action encoding rather than a private copy,
        // so the checksum cannot drift from the documented bit layout.
        for actions in inputs.active() {
            checksum = (checksum ^ u64::from(actions.bits())).wrapping_mul(PRIME);
        }
        self.checksum = checksum;
        self.tick += 1;
    }
}

/// Observation record for the fixed rules path.
///
/// Opt-in: the production plugin never inserts it, because `stages` and
/// `consumed_inputs` grow once per fixed tick and would climb without bound
/// across a 60 Hz session. Tests that want to observe the schedule insert it
/// themselves, and the recording systems skip the work when it is absent.
#[derive(Debug, Default, Resource)]
pub struct SimulationProbe {
    pub produced: u64,
    pub consumed: u64,
    pub stages: Vec<FixedStage>,
    pub consumed_inputs: Vec<TickInputs>,
}

#[derive(Debug, Default)]
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_duration(Duration::from_secs_f64(
            1.0 / FIXED_HZ,
        )))
        .init_resource::<CurrentTickInputs>()
        .init_resource::<RuleState>()
        .configure_sets(
            FixedUpdate,
            (FixedGameSet::Input, FixedGameSet::Rules)
                .chain()
                .run_if(in_state(AppState::Match)),
        )
        .add_systems(FixedUpdate, prepare_tick_inputs.in_set(FixedGameSet::Input))
        .add_systems(FixedUpdate, advance_rules.in_set(FixedGameSet::Rules));
    }
}

/// Sample local inputs, normalize them through `game_core`, and publish the
/// canonical `TickInputs` for this fixed tick.
pub fn prepare_tick_inputs(
    mut sampler: ResMut<LocalInputSampler>,
    mut current: ResMut<CurrentTickInputs>,
    probe: Option<ResMut<SimulationProbe>>,
) {
    let canonical: Vec<PlayerActions> = sampler
        .sample_fixed()
        .into_iter()
        .map(PlayerActions::normalized)
        .collect();

    current.inputs = TickInputs::new(&canonical).unwrap_or_else(|error| {
        warn!("dropping local inputs for this tick: {error}");
        TickInputs::EMPTY
    });
    current.consumed = false;

    if let Some(mut probe) = probe {
        probe.produced += 1;
        probe.stages.push(FixedStage::Input);
    }
}

/// Consume this tick's inputs exactly once and advance the rule state.
pub fn advance_rules(
    mut current: ResMut<CurrentTickInputs>,
    mut rules: ResMut<RuleState>,
    simulation: Option<ResMut<RulesSimulation>>,
    probe: Option<ResMut<SimulationProbe>>,
) {
    if current.consumed {
        return;
    }
    current.consumed = true;

    match simulation {
        Some(mut simulation) => {
            if let Err(error) = simulation.0.step(&current.inputs) {
                warn!("rules refused this tick: {error}");
            }
            rules.tick = simulation.0.match_tick();
            rules.checksum = simulation.0.checksum().0;
        }
        None => rules.advance(&current.inputs),
    }

    if let Some(mut probe) = probe {
        probe.consumed += 1;
        probe.consumed_inputs.push(current.inputs);
        probe.stages.push(FixedStage::Rules);
    }
}
