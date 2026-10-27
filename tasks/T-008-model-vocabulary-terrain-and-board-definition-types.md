# T-008: Model: vocabulary, terrain and board definition types

**Task T-008** · M0 Foundations · Est. core lines: ~300 / 500
**Depends on:** T-002, T-005

## Goal

The frozen Rust shapes of the board-side definitions.

## Design references

- `docs/design/strata-engine.md` §10.1 Conventions
- `docs/design/strata-engine.md` §10.2 Vocabulary
- `docs/design/strata-engine.md` §10.3 Terrain, tiles and features
- `docs/design/strata-engine.md` §11.1 Map

## Scope

- `SourceLoc`, `Text`, and definition structs `StatDef`, `DieDef` (symbol faces or numeric range), `MarkerDef`, `TraitsDef`, `FactionDef`, `TerrainDef`, `TileDef`, `FeatureDef` (cells with segments, `top` flag, `blocks_los`), `FootprintDef` (cells, move mode, max spread), `MapDef` (pieces, features, zones, relic spots, constraints), `InventoryDef`.
- Abilities attached to terrain are stored as an opaque `Vec<AbilityRef>` placeholder until T-009.
- All types derive serde, `Clone`, `Debug` and `StableHash`.

## Acceptance criteria

- [ ] Field names and units match §10 and §11.1.

## Tests

- serde round-trip for each type from hand-written JSON fixtures in `crates/strata_model/tests/fixtures/`.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
