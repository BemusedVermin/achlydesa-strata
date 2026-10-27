# T-002: Kernel: hex coordinates, distances, rotations and footprints

**Task T-002** · M0 Foundations · Est. core lines: ~300 / 500
**Depends on:** T-001

## Goal

Give every other crate one exact, integer hex vocabulary.

## Design references

- `docs/design/strata-engine.md` §7.1 Coordinates
- `docs/design/strata-engine.md` §7.3 Footprints

## Scope

- `Hex { q: i32, r: i32 }` with `s()`, the six neighbor directions in a fixed documented order, `neighbors()`, `distance()`, rotation by k×60° about the origin and about a center, reflection across an axis (for the editor's symmetry tool), `ring(radius)` and `spiral(radius)` in a deterministic order.
- `Space { q, r, z }` (z = level) and `Rotation(u8)` in 0..6.
- `Shape` (a list of axial offsets) with `rotated`, `translated`, `is_connected`, and a `common` module of named shapes (one, two-line, three-line, triangle, seven-hex flower).
- Derive `Ord` so `Hex` and `Space` work as `BTreeMap` keys; serde support.

## Acceptance criteria

- [ ] All operations are integer-only.
- [ ] Shapes rotate and translate without changing their cell count or connectivity.

## Tests

- proptest: distance is symmetric and satisfies the triangle inequality; every neighbor is at distance 1; rotation preserves distance; six 60° rotations are the identity; reflection twice is the identity; ring k has 6k cells for k ≥ 1.

## Out of scope

- Line traversal (T-006).

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
