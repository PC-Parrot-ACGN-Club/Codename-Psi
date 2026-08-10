//! LAN P2P session layer built on GGRS and UDP.
//!
//! Depends on `core` only for match state and sync primitives. Exact `bevy_ggrs`
//! client integration is deferred until the R2 compatibility prototype meets the
//! adoption criteria in `docs/TDD.md`.

#![forbid(unsafe_code)]

use core::MatchState;

/// Placeholder session handle until the GGRS prototype lands.
#[derive(Debug, Default)]
pub struct NetSession {
    _state: MatchState,
}
