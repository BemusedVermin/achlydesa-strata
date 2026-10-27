# T-006: Kernel: exact 2D segment traversal through the hex grid

**Task T-006** · M0 Foundations · Est. core lines: ~450 / 500
**Depends on:** T-002

## Goal

The exact-geometry heart of line of sight: which hex interiors a segment passes through, and where.

## Design references

- `docs/design/strata-engine.md` §7.5 Line of sight
- `docs/design/strata-engine.md` §7.6 Line of sight with sprites

## Scope

- Work in axial coordinates scaled by `LATTICE = 15`, so hex centers, hex corners (denominator 3) and the corners of 80% inset hexes (denominator 15) are all integer points. Document why 15.
- `Frac` exact rational (`i128` numerator and denominator, normalized, total order) for segment parameters t ∈ [0, 1].
- `traverse(a, b) -> Vec<Crossing { hex, t_in: Frac, t_out: Frac }>` in increasing t: every hex whose open interior the segment passes through. Grazing an edge or vertex does not count (§7.5 tie rule).
- `clip_inset(hex, a, b) -> Option<(Frac, Frac)>`: where the segment crosses the open interior of the 80% inset hexagon of `hex` (needed for figure volumes).
- Implement clipping as exact Cyrus–Beck against the six edge half-planes; enumerate candidate hexes without floats.

## Acceptance criteria

- [ ] No floating point anywhere.
- [ ] Crossings are contiguous, ordered, and cover the segment's path through interiors.

## Tests

- proptest: reversing the segment reverses the crossings (with t ↦ 1 − t); rotating both endpoints by 60° rotates every crossing hex; every endpoint strictly inside a hex appears first or last.
- Hand cases: center to center along each axis; a segment running exactly along a shared edge crosses neither neighbor's interior; vertex grazing.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
