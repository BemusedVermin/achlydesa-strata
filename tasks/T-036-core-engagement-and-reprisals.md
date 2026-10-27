# T-036: core:engagement and reprisals

**Task T-036** · M3 Standard rules and archetypes · Est. core lines: ~250 / 500
**Depends on:** T-034

## Goal

Who is engaged with whom, and what leaving costs.

## Design references

- `docs/design/strata-engine.md` §6.1 Slots
- `docs/design/strata-engine.md` §6.6 Queries, modifiers and descriptors

## Scope

- `is_engaged` query (default: adjacent enemies whose height ranges overlap, configurable).
- Leaving engagement offers each engaged enemy a reprisal roll with the configured die; `provokes_reprisal` query so abilities can deny it.
- Movement integration: `provokes_reprisal` flag on reachable spaces.

## Acceptance criteria

- [ ] A figure denied reprisal by a modifier triggers no rolls.

## Tests

- Pack tests for engagement across height differences, reprisals and denial.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
