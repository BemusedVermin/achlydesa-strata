# T-030: Toy ruleset with a slot swapped between Rust and Lua (M2 gate)

**Task T-030** · M2 Rules core · Est. core lines: ~200 / 500
**Depends on:** T-029, T-026

## Goal

Prove the core premise: any slot can be Rust or Lua with identical behavior.

## Design references

- `docs/design/strata-engine.md` §17 Build order (M2 gate)

## Scope

- `fixtures/packs/toy/`: two stats, one die, two terrains, two units, a 10×10 flat map.
- A minimal Rust `toy:combat` module in `strata_modules` and an equivalent Lua module in the toy pack.
- Two rulesets differing only in the combat slot; the same pack tests run against both.
- CI runs `strata pack test fixtures/packs/toy` (add a step to `ci.yml`; this task may edit `.github/workflows/`).

## Acceptance criteria

- [ ] Pack tests pass for both rulesets with identical event logs.

## Tests

- The pack tests themselves plus an event-log equality test.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
