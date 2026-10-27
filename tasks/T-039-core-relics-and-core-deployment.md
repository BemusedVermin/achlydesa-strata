# T-039: core:relics and core:deployment

**Task T-039** · M3 Standard rules and archetypes · Est. core lines: ~400 / 500
**Depends on:** T-033

## Goal

Claimable relics and the setup phase.

## Design references

- `docs/design/strata-engine.md` §10.6 Relics
- `docs/design/strata-engine.md` §11.2 Scenario
- `docs/design/strata-engine.md` §6.1 Slots

## Scope

- Relics placed on spots from the scenario, face-up or face-down; claiming on `ends_move_on`; `while_occupied` grants (abilities attached to the occupant's army) and `on_claim` effects.
- Deployment: start zones per slot, placement order, a placement command validated against zones and surfaces.

## Acceptance criteria

- [ ] A `while_occupied` bonus disappears exactly when the occupant leaves.

## Tests

- Pack tests for claiming, face-down reveal, bonus on and off, and illegal placement.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
