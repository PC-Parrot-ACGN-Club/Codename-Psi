# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Codename Psi — a local Fever-style (Puyo Puyo-esque) versus game, Rust + Bevy. The project is at an early engineering-baseline stage: crate skeletons and stub RON/JSON config loaders exist, but the deterministic rules engine, application state machine, and gameplay systems are not yet implemented. Right now most of the project's substance lives in `docs/`, which is written *before* and constrains implementation — treat it as a binding contract, not background reading.

## Commands

- Build: `cargo build` (builds default member `client` → binary `psi`; explicit form `cargo build -p client --bin psi`)
- Run: `cargo run` (launches the Bevy window; `assets/` must be reachable from the working directory the binary is launched from — repo root in development)
- Test all: `cargo test`
- Test one crate: `cargo test -p game_core`
- Format check: `cargo fmt --check`
- Toolchain is pinned to `1.97.1` via `rust-toolchain.toml` (installs `rustfmt` + `clippy`).
- **CI only runs on pull requests targeting `main`** (`.github/workflows/test.yml`). Pushes to development branches are not verified by CI at all — this is a deliberate trade-off against the organization's Actions quota (`docs/TDD.md` §7.2). Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` locally before committing; nothing else will catch a regression until the `main` PR.
- That PR job runs with `RUSTFLAGS=-D warnings`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `cargo test -p game_core`, `cargo test --workspace`, and a build for `x86_64-unknown-linux-gnu` (ubuntu job, installs Bevy's Linux system libs).
- Target platform for R1 and R2 is **Linux only** — `x86_64-unknown-linux-gnu` is the sole build target across docs, `test.yml` and `release.yml`. Other targets are out of scope until R2 ships; don't add build matrix entries for them without a corresponding PRD/TDD change.
- `release.yml` is `workflow_dispatch` only: it builds the production client with `--release` (test design TC-049) and runs no tests.

## Architecture

### Workspace crates and dependency direction

- `crates/game_core` — pure rules engine: match state, config models, deterministic helpers. `#![forbid(unsafe_code)]`. Must stay free of Bevy, windowing, networking, and filesystem I/O; callers pass already-loaded bytes/strings in.
- `crates/client` — the Bevy app binary (`psi`). Owns windowing, rendering, native UI, device input, audio, and asset paths.
- `crates/net` — LAN P2P session layer (GGRS + UDP), an optional feature of `client`. Depends on `game_core` only.

Dependency direction is fixed: `client → game_core`, `net → game_core`. `game_core` never depends back on `client` or `net`. This boundary is load-bearing for the whole determinism/testing strategy below, not just style preference.

### Determinism model

- Rules run on a fixed 60Hz tick; Bevy rendering/UI run at their own frame rate and read the latest rule snapshot to animate.
- Tick input is a per-player action bitset: left, right, soft drop, hard drop, rotate CW, rotate CCW. Client-side interactions (confirm/back/pause) are handled outside the rule tick, in client input context.
- Ball sequences, garbage-column allocation, and Fever board selection all derive from an explicit RNG seed set at match init, recorded for deterministic verification metadata (a dev/CI tool, not a player-facing replay feature — see PRD §2.3).
- Rule-affecting state (board, active/next pieces, timers, score, garbage queue, Fever state, win/loss, RNG state) must be reproducible: same initial state + input log ⇒ same state checksum.
- Render entities, audio playback, particles, user settings, and network sockets are presentation/infrastructure and must never enter rollback-able rule state — this matters for the future GGRS rollback integration.

### Application state machine (design landed, not yet implemented)

Top-level flow, owned solely by `client::app_state` via Bevy `States` (other client components request transitions, they don't hold parallel top-level state):

```
Boot → MainMenu → ModeSelect → CharacterSelect → Match ⇄ Paused → Result → MainMenu
```

`Boot` is a synchronous barrier: it waits for both `UserSettings` and `Localization` to resolve (success or safe-default fallback both count as `Resolved`) before requesting `Boot → MainMenu`. Everything from `MainMenu` onward may assume both are available. Full spec: `docs/development/system/game-infrastructure-architecture.md` and `docs/development/component/application-state-machine.md`.

### Config & assets

- `assets/data/*.ron` — rules, characters, Fever boards. `assets/i18n/{en,zh-CN}.json` — UI strings keyed by stable ids, missing keys fall back to English.
- Every versioned data file carries a top-level `schema_version`; loaders reject unsupported versions with a typed error (`ConfigError` in `crates/game_core/src/config.rs`). Files currently under `assets/` are parse fixtures only — full rule profiles land with the deterministic rules kernel.

## Documentation-driven workflow

Design and test-case docs are reviewed and confirmed *before* implementation. When code changes affect documented behavior, update the corresponding doc in the same change so docs, code, config, and acceptance criteria stay consistent.

- `docs/PRD.md`, `docs/gameplay.md`, `docs/presentation.md`, `docs/TDD.md` are the founding contracts (product scope, gameplay rules, presentation/UI, tech stack + crate boundaries + determinism + CI). Amend in place when reality diverges from them.
- `docs/development/` holds design docs classified by assembly scope — see `docs/development/README.md`:
  - **Component** — single module, data model, rule, or local state/behavior.
  - **Component Integration** — interface/contract/protocol between ≥2 components.
  - **System** — multi-module architecture, runtime lifecycle, or user-facing flow.
  - Document types within a category: Spec, Contract, Architecture, Flow, Decision.
- `docs/test/design/` holds test-case designs, reviewed before implementation, classified along three independent axes (see `docs/test/README.md`):
  - **Test Level** — Component → Component Integration → System; pick the smallest scope that already proves the behavior.
  - **Concern** — cross-cutting verification purpose (Smoke, Content Validation, Determinism, ...).
  - **Domain** — gameplay responsibility area (Rules, Match Flow, Configuration, Client, Input, AI, Network).
- Every design/test conclusion carries an evidence status: `Confirmed` (direct evidence from files/tests/config/git history), `Inferred` (reasoned from evidence, needs user confirmation), or `Unknown` (characterization test protecting current behavior only, revisit once design is confirmed).

Docs are written in Chinese; code, comments, and identifiers are in English.

## Git

Always keep commit at local space, push or pr creation require manual autorization.

Work on worktree only when the work is about codding.

For documentation works, user confirmation on changes is required before committing, isolated worktree is not needed.