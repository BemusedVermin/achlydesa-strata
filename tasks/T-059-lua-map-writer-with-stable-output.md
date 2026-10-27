# T-059: Lua map writer with stable output

**Task T-059** · M5 Map editor · Est. core lines: ~250 / 500
**Depends on:** T-057

## Goal

Save maps as clean, diffable Lua.

## Design references

- `docs/design/strata-engine.md` §11.1 Map
- `docs/design/strata-engine.md` §14.1 Editor core (Writer)

## Scope

- Serialize `MapDef` to a `return map { ... }` Lua literal with stable key order and one piece per line, matching the style in §11.1.
- Round trip: write, load through the data stage, compare.

## Acceptance criteria

- [ ] Writing the same map twice produces byte-identical files.

## Tests

- Round-trip tests on fixture maps; snapshot of written output.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
