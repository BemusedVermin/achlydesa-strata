# T-026: Rules: windows and reactions

**Task T-026** · M2 Rules core · Est. core lines: ~250 / 500
**Depends on:** T-023

## Goal

Named pauses inside a resolution where other players may react.

## Design references

- `docs/design/strata-engine.md` §6.7 Triggers, replacements and windows

## Scope

- `ctx.window(id)`: collect eligible reactions (registered per window), offer them to their controllers as decisions, resolve chosen reactions in order.
- Windows declared by modules; reactions registered by abilities.

## Acceptance criteria

- [ ] A window with no eligible reactions costs no decision.

## Tests

- Two reactions from different players in one window; a declined reaction; nested window guard.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
