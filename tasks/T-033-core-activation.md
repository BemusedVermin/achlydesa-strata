# T-033: core:activation

**Task T-033** · M3 Standard rules and archetypes · Est. core lines: ~300 / 500
**Depends on:** T-032

## Goal

A unit's turn: steps, slots and ordering, configured by the ruleset.

## Design references

- `docs/design/strata-engine.md` §6.5 Actions
- `docs/design/strata-engine.md` §6.3 Rulesets

## Scope

- Steps from config (default move then attack), per-figure slots, `all_figures_per_step` ordering, ending an activation early.
- Hooks for abilities granting extra slots.

## Acceptance criteria

- [ ] A squad moves all figures before any attacks under the default config.

## Tests

- Pack tests for ordering, early end and a granted extra move.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
