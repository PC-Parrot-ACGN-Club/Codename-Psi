//! Pure rules engine: match state, config models, tick input, and replay.
//!
//! This crate must stay free of Bevy, windowing, networking, and filesystem I/O.

#![forbid(unsafe_code)]

/// Placeholder until the deterministic match state machine lands.
#[derive(Debug, Clone, Default)]
pub struct MatchState;
