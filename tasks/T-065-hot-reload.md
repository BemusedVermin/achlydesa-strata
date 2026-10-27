# T-065: Hot reload

**Task T-065** · M6 AI and tooling · Est. core lines: ~350 / 500
**Depends on:** T-056

## Goal

See a pack edit in a running match within seconds.

## Design references

- `docs/design/strata-engine.md` §16.2 Hot reload

## Scope

- Dev builds watch loaded pack folders with `notify`; on change, reload; if valid, swap the registry and replay the match log to the same command index, stopping at the last legal command.
- Load errors appear as an in-game overlay with file and line.

## Acceptance criteria

- [ ] Editing a unit's stat updates its card without restarting.

## Tests

- A session-level test: reload with a changed stat and verify the replayed state.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
