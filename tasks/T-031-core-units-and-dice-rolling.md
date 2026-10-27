# T-031: core:units and dice rolling

**Task T-031** · M3 Standard rules and archetypes · Est. core lines: ~350 / 500
**Depends on:** T-030

## Goal

Heroes, squads, wounds and destruction, plus dice pools.

## Design references

- `docs/design/strata-engine.md` §6.1 Slots
- `docs/design/strata-engine.md` §6.8 Dice

## Scope

- `core:units`: unit kinds as module vocabulary, `ctx.units.wound`, `heal`, `destroy`, removal of a unit when its last figure is destroyed, `FigureDestroyed` events.
- Dice: roll pools from `DieDef`s through the command RNG, `DiceRolled` events, `DicePool` query, reroll as decision plus replacement roll.
- Lua namespaces `ctx.units` and `ctx.dice`.

## Acceptance criteria

- [ ] Wounds at or above life destroy a figure exactly once.

## Tests

- Pack tests in a fixture pack for wounding, destruction and rerolls.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
