# T-062: Greedy AI controller and evaluator terms

**Task T-062** · M6 AI and tooling · Est. core lines: ~450 / 500
**Depends on:** T-046

## Goal

A competent one-ply opponent that works for modded abilities automatically.

## Design references

- `docs/design/strata-engine.md` §15.2 AI

## Scope

- `AiController`: for each legal command, clone, resolve with a runtime Lua state, score, pick the best; deterministic tie-breaking.
- Evaluator terms registered by modules: material (points weighted by wounds), threat (expected damage next turn from exact dice distributions), objective progress from victory conditions.
- Decisions answered by shallow simulation with a budget, falling back to the first option.
- `rayon` workers, each owning a runtime Lua state.

## Acceptance criteria

- [ ] The AI beats `RandomController` in at least 90% of 200 seeded games on the standard scenario.
- [ ] Same seed, same choices.

## Tests

- Determinism test; the win-rate test (may be `#[ignore]`d in normal runs and executed in CI's determinism job).

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
