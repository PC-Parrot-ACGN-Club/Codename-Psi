//! Pure rules engine: match state, config models, tick input, and replay.
//!
//! # Crate boundary
//!
//! This crate must stay free of Bevy, windowing, networking, and filesystem I/O.
//! Callers pass already-loaded bytes or strings; `client` owns disk/asset paths.

#![forbid(unsafe_code)]

pub mod board;
pub mod config;
pub mod control;
pub mod determinism;
pub mod digest;
pub mod drop_stream;
pub mod falling;
pub mod fever;
pub mod input;
pub mod match_spec;
pub mod match_state;
pub mod nuisance;
pub mod player;
pub mod resolution;
pub mod rules;
pub mod safety_point;
pub mod scoring;

pub use match_state::MatchState;
