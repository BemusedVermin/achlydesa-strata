# T-029: Pack test harness and `strata pack test`

**Task T-029** · M2 Rules core · Est. core lines: ~450 / 500
**Depends on:** T-028

## Goal

Let packs prove their rules with fast, rigged-dice tests.

## Design references

- `docs/design/strata-engine.md` §16.4 Pack tests

## Scope

- `test {}` constructor: ruleset, map, `place` (put figures directly, skipping deployment), `steps`.
- `act.*` helpers that submit commands and answer decisions; `rig` that forces the next dice faces; `expect.*` helpers for stats, events, figure state and decisions.
- Figure handles like `p0.unit.figure#2` resolved by the harness.
- Rust runner API used by `cargo test` and the `strata pack test <path>` subcommand with a readable summary and nonzero exit on failure.

## Acceptance criteria

- [ ] Failures report the test name, the step index and the expected versus actual values.

## Tests

- Harness tests against a fixture pack, including a deliberately failing test whose output is snapshotted.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
