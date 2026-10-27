# T-061: Editor UI: slicing, zones, relics, symmetry, validation, playtest (M5 gate)

**Task T-061** · M5 Map editor · Est. core lines: ~450 / 500
**Depends on:** T-060, T-058, T-059

## Goal

Finish the editor and close the loop from building to playing.

## Design references

- `docs/design/strata-engine.md` §14.2 Editor UI
- `docs/design/strata-engine.md` §14.3 Scenarios from a map
- `docs/design/strata-engine.md` §17 Build order (M5 gate)

## Scope

- Level slicing control; zone painter with named, colored layers; relic-spot tool.
- Symmetry tool (mirror or rotate placements for 2 to 6 players) using kernel reflection and rotation.
- Validation panel; clicking a finding frames its cells.
- Save via the Lua writer; generate a scenario skeleton; playtest button starting a hotseat match on the current map.

## Acceptance criteria

- [ ] A map can be built, validated, saved and playtested without leaving the app.

## Tests

- Unit tests for symmetry mapping and skeleton generation.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
