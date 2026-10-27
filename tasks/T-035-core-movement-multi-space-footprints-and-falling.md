# T-035: core:movement: multi-space footprints and falling

**Task T-035** · M3 Standard rules and archetypes · Est. core lines: ~400 / 500
**Depends on:** T-034

## Goal

Two-space and larger figures, in both move modes, and fall damage.

## Design references

- `docs/design/strata-engine.md` §7.3 Footprints
- `docs/design/strata-engine.md` §7.4 Movement

## Scope

- Search over (anchor space, rotation) nodes; every cell must have a surface within `max_spread` levels.
- `snake` mode (rear follows the leader's path) and `rigid` mode (shape translates).
- Falling: a configurable rule emitting wounds via `ctx.units`.

## Acceptance criteria

- [ ] A two-space figure never ends with a cell over empty space.

## Tests

- Pack tests for snake turns, rigid translation, spread limits and falling.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
