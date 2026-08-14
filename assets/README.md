# Assets

Runtime data for Codename Psi. Bevy resolves this directory's parent from `BEVY_ASSET_ROOT`, then `CARGO_MANIFEST_DIR`, then the executable's directory — never the working directory. See `docs/development/design/runtime-data-loading.md`.

## Layout

Rule data is split into two independently versioned parts: a **rule profile** says how one set of competitive rules computes, and a **content library** says what is selectable under that profile.

| Path | Format | Purpose |
| --- | --- | --- |
| `data/rules/profiles/<profile_id>.ron` | RON | One rule profile: `field`, `round`, `drop`, `rotation`, `resolve`, `scoring`, `offense`, `nuisance`, `fever` |
| `data/rules/roster.ron` | RON | Character identities |
| `data/rules/play/<profile_id>/<character_id>.ron` | RON | That character's drop set and chain-power curves under that profile |
| `data/rules/puzzles/<profile_id>.ron` | RON | Fever puzzle book for that profile |
| `i18n/zh-CN.json`, `i18n/en.json` | JSON | UI strings keyed by stable ids |

Every duration is written in ticks at the fixed 60 Hz rules rate; seconds appear only in comments. Tables derived from parameters (margin target-score decay, chain-power curves) are written as integer tables: the table is the authority and the parameters beside it are provenance, checked against each other offline.

## Schema version

Every versioned data file carries a top-level `schema_version` integer.

- Loaders reject unknown or unsupported versions with a typed error.
- Player-facing UI for those errors is out of scope for the engineering baseline; development builds surface the error message from the typed failure.

Stub files in this tree are parse fixtures only. Full rule profiles land with the deterministic rules kernel.
