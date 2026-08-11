//! Pure rules engine: match state, config models, tick input, and replay.
//!
//! # Crate boundary
//!
//! This crate must stay free of Bevy, windowing, networking, and filesystem I/O.
//! Callers pass already-loaded bytes or strings; `client` owns disk/asset paths.

#![forbid(unsafe_code)]

pub mod config;
pub mod input;

/// Placeholder until the deterministic match state machine lands.
#[derive(Debug, Clone, Default)]
pub struct MatchState;
