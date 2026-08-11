//! Ordered 60 Hz fixed-update bridge into the rules layer.

use std::time::Duration;

use bevy::prelude::*;
use game_core::input::TickInputs;

use crate::app_state::AppState;

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

#[derive(Debug, Default, Resource)]
pub struct SimulationProbe {
    pub produced: u64,
    pub consumed: u64,
    pub rule_ticks: u64,
    pub stages: Vec<FixedStage>,
    pub current_inputs: TickInputs,
}

#[derive(Debug, Default)]
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_duration(Duration::from_secs_f64(
            1.0 / FIXED_HZ,
        )))
        .init_resource::<SimulationProbe>()
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

pub fn prepare_tick_inputs(mut probe: ResMut<SimulationProbe>) {
    probe.produced += 1;
    probe.stages.push(FixedStage::Input);
}

pub fn advance_rules(mut probe: ResMut<SimulationProbe>) {
    probe.consumed += 1;
    probe.rule_ticks += 1;
    probe.stages.push(FixedStage::Rules);
}
