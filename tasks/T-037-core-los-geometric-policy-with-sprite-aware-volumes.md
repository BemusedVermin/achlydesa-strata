# T-037: core:los: geometric policy with sprite-aware volumes

**Task T-037** · M3 Standard rules and archetypes · Est. core lines: ~400 / 500
**Depends on:** T-007, T-031

## Goal

Line of sight that matches what players see on screen, computed without a camera.

## Design references

- `docs/design/strata-engine.md` §7.5 Line of sight
- `docs/design/strata-engine.md` §7.6 Line of sight with sprites

## Scope

- Build blockers per column from terrain segments, LOS-blocking features and figures (80% inset prisms over their footprint cells, from the hit zone's bottom to top).
- 21 sample points per one-space figure (three heights × center plus six inset corners), more for larger footprints; the attacker's target point at `target` above its anchor center.
- `LineOfSight` service returning `{ visible, clear_fraction, witnesses }`; policies `geometric` (threshold `any` by default) and `columns` (center to center).
- `in_los` selector filter wired to the service.

## Acceptance criteria

- [ ] LOS between two figures is invariant under rotating the whole board by 60°.

## Tests

- Pack tests with walls, bridges, partial cover over a low wall, and intervening figures.
- proptest rotation invariance on generated small boards.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
