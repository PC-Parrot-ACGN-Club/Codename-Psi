# Codename Psi

Local Fever-style versus game (working title). Engineering baseline for a Rust / Bevy workspace.

## Requirements

- Rust toolchain from [`rust-toolchain.toml`](rust-toolchain.toml) (`1.97.1`, with `rustfmt` and `clippy`)
- Repository `assets/` as the data root when launching from the workspace root

## Workspace

| Crate | Role |
| --- | --- |
| `core` | Pure rules, config models, deterministic helpers (no Bevy / FS / net) |
| `client` | Bevy app binary `psi` |
| `net` | LAN P2P session layer (optional client feature) |

Dependency direction: `client → core`, `net → core`.

## Local checks

From the repository root:

```bash
cargo fmt --check
cargo test
cargo build
```

`cargo build` builds the default member (`client` → binary `psi`). Equivalent explicit form: `cargo build -p client --bin psi`.

Run the client (window + `Camera2d` smoke startup):

```bash
cargo run
```

## Assets

See [`assets/README.md`](assets/README.md) for layout, `schema_version`, and stub fixtures.
