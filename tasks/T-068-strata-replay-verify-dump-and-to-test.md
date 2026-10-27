# T-068: `strata replay verify`, `dump` and `to-test`

**Task T-068** · M6 AI and tooling · Est. core lines: ~300 / 500
**Depends on:** T-044, T-019

## Goal

Tools for bug reports and regression tests.

## Design references

- `docs/design/strata-engine.md` §12.6 Saves and replays
- `docs/design/strata-engine.md` §15.4 Replays

## Scope

- `verify`: re-resolve every command and compare each state hash; report the first mismatch.
- `dump`: readable text of setup, commands, answers and events.
- `to-test --at N`: write a pack test that starts from the state at command N and asserts the recorded outcome.

## Acceptance criteria

- [ ] A replay from T-044's tests verifies cleanly; a tampered one reports the exact command.

## Tests

- Snapshot tests of `dump`; round trip of `to-test`.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
