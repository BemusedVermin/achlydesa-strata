# T-054: Decision UI and command input

**Task T-054** · M4 Playable client · Est. core lines: ~450 / 500
**Depends on:** T-053, T-048

## Goal

Every decision and command, with no per-ability UI code.

## Design references

- `docs/design/strata-engine.md` §13.5 Generic decision UI
- `docs/design/strata-engine.md` §13.3 Plugins (InputPlugin)

## Scope

- Decision widgets: board highlighting for figure and space choices, dialog for options, stepper for numbers, drag list for ordering, yes/no.
- Command building from `legal_commands`: select unit, select figure, select action, select target; invalid clicks explain why.
- Token placement screen.

## Acceptance criteria

- [ ] A Lua ability's `ctx.choose` with figures highlights exactly the legal figures.

## Tests

- Unit tests for decision-to-widget mapping and command building from legal commands.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
