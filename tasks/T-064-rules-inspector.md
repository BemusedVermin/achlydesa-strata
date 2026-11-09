# T-064: Rules inspector

**Task T-064** · M6 AI and tooling · Est. core lines: ~300
**Depends on:** T-022, T-053

## Goal

Answer "why is this number 5?" for any query.

## Design references

- `docs/design/strata-engine.md` §16.6 Rules inspector

## Scope

- Collect query traces (from T-022) on demand for a selected figure and stat.
- Dev panel showing base value, each modifier with source and whether it ran natively or in Lua, clamping, and the result.

## Acceptance criteria

- [ ] The trace's arithmetic reproduces the displayed value.

## Tests

- Unit test for trace formatting.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
