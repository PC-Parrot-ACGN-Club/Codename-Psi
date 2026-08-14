//! Pure rules engine: match state, config models, tick input, and replay.
//!
//! # Crate boundary
//!
//! This crate must stay free of Bevy, windowing, networking, and filesystem I/O.
//! Callers pass already-loaded bytes or strings; `client` owns disk/asset paths.

#![forbid(unsafe_code)]

pub mod board;
pub mod config;
pub mod input;
pub mod nuisance;
pub mod resolution;
pub mod rules;
pub mod scoring;

/// Placeholder aggregation root until match/round orchestration lands.
///
/// The component modules intentionally remain usable without this type: they
/// are pure, in-memory building blocks for the rules-engine test suite.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MatchState;
