# T-007: Kernel: 3D occlusion test for columns and figure volumes

**Task T-007** · M0 Foundations · Est. core lines: ~350 / 500
**Depends on:** T-006

## Goal

Decide exactly whether a 3D segment is blocked, given what stands in each column.

## Design references

- `docs/design/strata-engine.md` §7.5 Line of sight
- `docs/design/strata-engine.md` §7.6 Line of sight with sprites

## Scope

- `Seg3`: two points with lattice x/y and z in sub-level units (12 per level).
- `Blocker { footprint: FullHex | InsetHex, z0, z1 }` (half-open, sub-level units).
- `blocked(seg, blockers: impl Fn(Hex) -> &[Blocker], ignore: &[Hex]) -> bool`: for each crossing, compute the segment's height interval over the crossing (exact, linear in t) and test overlap with the open interior of every blocker; inset blockers use `clip_inset`. Endpoint columns listed in `ignore` are skipped.
- `clear_count(origin, targets, ...)` returning how many target samples are unblocked, plus the clear segments as witnesses.

## Acceptance criteria

- [ ] Touching a blocker's top, bottom or side does not block.
- [ ] Pure integer and rational arithmetic.

## Tests

- Hand-built cases: a wall of height 3 between two figures blocks low samples but not high ones; a bridge deck blocks a segment passing through it but not one passing under it.
- proptest: the result is invariant under 60° rotation of the whole scene and under swapping segment endpoints.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
