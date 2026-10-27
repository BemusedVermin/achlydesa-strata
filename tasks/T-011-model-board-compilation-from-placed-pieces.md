# T-011: Model: board compilation from placed pieces

**Task T-011** · M0 Foundations · Est. core lines: ~400 / 500
**Depends on:** T-008

## Goal

Turn a map's placed pieces and features into the column structure all geometry runs on.

## Design references

- `docs/design/strata-engine.md` §7.2 Columns, segments and surfaces
- `docs/design/strata-engine.md` §11.1 Map

## Scope

- `CompiledBoard`: dense grid over the bounding box; per column an ordered list of `Segment { z0, z1, terrain }`, derived surfaces with headroom, and feature segments with their flags.
- Compile `MapDef` + tile/feature lookups: apply rotation and level; error on overlapping pieces; enforce `require_support`.
- Queries: `column(hex)`, `surfaces(hex)`, `top_surface(hex)`, `is_solid(hex, z)`, `blockers(hex)` producing kernel `Blocker`s for LOS.

## Acceptance criteria

- [ ] A bridge (ground 0–1, deck 4–5) yields two surfaces at the same hex.
- [ ] Errors carry the offending piece index and hex.

## Tests

- Overlap and support errors; bridge surfaces; rotating every piece of a map by 60° yields the rotated board.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
