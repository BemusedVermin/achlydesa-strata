# T-034: core:movement: reachable() for one-space figures

**Task T-034** · M3 Standard rules and archetypes · Est. core lines: ~450 / 500
**Depends on:** T-033, T-011

## Goal

Native, deterministic pathfinding that every preview and the AI read.

## Design references

- `docs/design/strata-engine.md` §7.4 Movement
- `docs/design/strata-engine.md` §7.2 Columns, segments and surfaces

## Scope

- `reachable(figure)`: Dijkstra over surfaces with integer costs from `move_cost`, `can_enter` and `can_climb` queries; deterministic tie-breaking by `Space` order.
- Edge-cost cache per (figure, terrain, level change) for one query (§7.4).
- Flags: `stops_here` (stop-on-enter terrain), `falls` (descending beyond the configured limit), `occupied`; headroom check (`ignore` or `figure_height`).
- The ordinary `move` action using the result; `FigureMoved` with the full path.

## Acceptance criteria

- [ ] Results are identical regardless of query order or cache state.

## Tests

- Pack tests on fixture maps: climbing limits, stop-on-enter, bridges (moving under and over), occupied spaces.
- proptest: path cost is never less than hex distance.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
