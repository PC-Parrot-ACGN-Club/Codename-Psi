# Assets

Runtime data for Codename Psi. The game expects this directory next to the working directory used to launch the binary (repository root during development).

## Layout

| Path | Format | Purpose |
| --- | --- | --- |
| `data/*.ron` | RON | Rules, characters, Fever boards (schema evolves in later milestones) |
| `i18n/zh-CN.json`, `i18n/en.json` | JSON | UI strings keyed by stable ids |

## Schema version

Every versioned data file carries a top-level `schema_version` integer.

- Loaders reject unknown or unsupported versions with a typed error.
- Player-facing UI for those errors is out of scope for the engineering baseline; development builds surface the error message from the typed failure.

Stub files in this tree are parse fixtures only. Full rule profiles land with the deterministic rules kernel.
