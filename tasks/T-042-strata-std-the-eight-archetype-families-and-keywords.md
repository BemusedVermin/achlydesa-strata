# T-042: strata_std: the eight archetype families and keywords

**Task T-042** · M3 Standard rules and archetypes · Est. core lines: ~450 / 500
**Depends on:** T-041

## Goal

Make most abilities one-line archetype calls.

## Design references

- `docs/design/strata-engine.md` §8.1 The eight families
- `docs/design/strata-engine.md` §8.3 Archetypes in use
- `docs/design/strata-engine.md` §8.5 Unique abilities

## Scope

- `packs/strata_std/abilities/`: `stat_bonus`, `aura`, `special_attack`, `movement_mode`, `reaction`, `roll_trigger`, `activation_trick`, `board_effect`, each building an `ability {}` definition from descriptors and recording `ArchetypeInfo`.
- `hazard_roll` terrain archetype (a `board_effect` variant using `sel.occupant`).
- `packs/strata_std/lib/keywords.lua` with a handful of original keywords (for example `flight`, `steady`, `disciplined`) that accept overrides.
- Extend `strata stubs` with archetype signatures.

## Acceptance criteria

- [ ] Each archetype produces abilities that evaluate natively (no `Logic::Script`) unless a Lua function is passed in.

## Tests

- Pack tests: at least two per archetype, including a mixed archetype with one Lua function parameter.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
