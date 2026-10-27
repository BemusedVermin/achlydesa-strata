# T-043: strata_std: base content and eight test units

**Task T-043** · M3 Standard rules and archetypes · Est. core lines: ~150 / 500
**Depends on:** T-042, T-039, T-040

## Goal

A playable standard pack.

## Design references

- `docs/design/strata-engine.md` §10.2 Vocabulary
- `docs/design/strata-engine.md` §6.3 Rulesets

## Scope

- Stats, combat die and d20, wound marker, traits vocabulary, terrains (grass, rock, sand, water, magma), tiles in 1/3/7/24 shapes, one- and two-space footprints.
- Rulesets `advanced` and `intro` exactly as in §6.3.
- A 12×12 test map with height, a bridge and water; one scenario; one format.
- Eight units (heroes and squads, one two-space) with neutral names and archetype abilities; `assets.lua` with no sprites (fallbacks render).

## Acceptance criteria

- [ ] `strata pack validate packs/strata_std` passes; every unit appears in at least one pack test.

## Tests

- Pack tests per unit ability.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
