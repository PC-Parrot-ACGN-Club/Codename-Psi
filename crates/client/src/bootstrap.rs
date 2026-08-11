//! Startup barrier for settings and localization.

use bevy::prelude::*;

use crate::app_state::{
    AppState, AppTransitionCause, AppTransitionRequests, arbitrate_transitions,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BootstrapTaskState {
    #[default]
    Pending,
    Resolved,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Resource)]
pub struct BootstrapStatus {
    pub settings: BootstrapTaskState,
    pub localization: BootstrapTaskState,
}

impl BootstrapStatus {
    #[must_use]
    pub const fn is_ready(self) -> bool {
        matches!(self.settings, BootstrapTaskState::Resolved)
            && matches!(self.localization, BootstrapTaskState::Resolved)
    }
}

#[derive(Debug, Default)]
pub struct BootstrapPlugin;

impl Plugin for BootstrapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BootstrapStatus {
            settings: BootstrapTaskState::Resolved,
            localization: BootstrapTaskState::Resolved,
        })
        .add_systems(
            Update,
            request_main_menu
                .before(arbitrate_transitions)
                .run_if(in_state(AppState::Boot)),
        );
    }
}

pub fn request_main_menu(
    status: Res<BootstrapStatus>,
    mut requests: ResMut<AppTransitionRequests>,
) {
    if status.is_ready() {
        requests.submit(AppState::MainMenu, AppTransitionCause::BootstrapReady);
    }
}
