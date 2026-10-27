# T-025: Rules: native descriptor evaluator (selectors, conditions, values)

**Task T-025** · M2 Rules core · Est. core lines: ~450 / 500
**Depends on:** T-022, T-011

## Goal

The fast path: evaluate descriptors in Rust so most abilities never enter Lua.

## Design references

- `docs/design/strata-engine.md` §6.6 Queries, modifiers and descriptors
- `docs/design/strata-engine.md` §8.2 Descriptor vocabulary

## Scope

- Typed `Descriptor` enum compiled from the `Value` tree at freeze, with clear errors for unknown tags.
- Selectors: `self`, `allies`, `enemies`, `my_units`, `occupant`, with filters `adjacent_to`, `within`, `of`, `trait`, `kind`. Results always sorted by ID.
- Conditions: `all`, `any`, `none`, `not`, comparisons, `count`, `phase`, `on_terrain`, `higher_than`.
- Values: literals, `stat`, `count`, `distance`, `level_difference`, `sum`, `min`, `max`.
- A card-text phrase for each descriptor (used by T-041).

## Acceptance criteria

- [ ] `LogicEvaluator` for `Logic::Native` is implemented; unknown descriptors fail at load, not at runtime.

## Tests

- One test per descriptor on a small compiled board fixture.

## Out of scope

- Effects (`eff.*`) and descriptors needing combat or LOS modules (T-041).

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
