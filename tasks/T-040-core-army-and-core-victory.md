# T-040: core:army and core:victory

**Task T-040** · M3 Standard rules and archetypes · Est. core lines: ~350 / 500
**Depends on:** T-038

## Goal

Army legality and how matches end.

## Design references

- `docs/design/strata-engine.md` §11.2 Scenario
- `docs/design/strata-engine.md` §11.3 Format
- `docs/design/strata-engine.md` §11.4 Army list

## Scope

- `army_rule.*` constructors: `max_points`, `unique_once`, `max_copies`, `banned`, `require_faction`, `max_footprint`; `validate(list, format, scenario)` returning all violations with their rule.
- `win.*` constructors: `eliminate_all`, `hold_zone`, `round_limit` with tiebreaks; evaluation after each command and at round end; optional `progress` estimate for the AI.

## Acceptance criteria

- [ ] A scenario's army rules and its format's rules both apply.

## Tests

- Validation fixtures; pack tests ending a match by each victory condition.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
