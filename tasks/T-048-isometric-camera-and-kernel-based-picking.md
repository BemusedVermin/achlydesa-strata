# T-048: Isometric camera and kernel-based picking

**Task T-048** · M4 Playable client · Est. core lines: ~350 / 500
**Depends on:** T-047

## Goal

The 2.5D view and exact hex picking.

## Design references

- `docs/design/strata-engine.md` §13.8 Isometric rendering and sprites
- `docs/design/strata-engine.md` §13.3 Plugins

## Scope

- Orthographic 3D camera at a configurable elevation (default 30°), six yaw steps of 60° with smooth turning, integer zoom steps, pan.
- World mapping from axial coordinates and levels (pointy or flat orientation from the map).
- Picking: cast the cursor ray against compiled-board surfaces with kernel math (no physics) to get the hovered space; `HoveredSpace` resource.

## Acceptance criteria

- [ ] The hovered space is correct on stacked terrain and under bridges from every yaw.

## Tests

- Unit tests for the ray-to-space math on a fixture board.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
