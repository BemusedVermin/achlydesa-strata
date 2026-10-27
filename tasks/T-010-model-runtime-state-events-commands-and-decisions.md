# T-010: Model: runtime state, events, commands and decisions

**Task T-010** · M0 Foundations · Est. core lines: ~450 / 500
**Depends on:** T-009

## Goal

Define the game state and make events its only mutator.

## Design references

- `docs/design/strata-engine.md` §5.1 Commands, events, decisions
- `docs/design/strata-engine.md` §12 Data model: runtime game state

## Scope

- Typed IDs `FigureId`, `UnitId`, `PlayerId`; `GameState`, `MatchSetup`, `PlayerSetup`, `PackIdentity`, `PlayerState`, `UnitState`, `FigureState` (location, rotation, status, controller, markers), `RelicState`, `AbilityInstance`, `FlowState` (phase stack, round, turn, active player, activation), `PendingDecision`, `BoardChanges`, `VarKey`.
- `Command` (place tokens, activate unit, action, end activation, custom) and `Event` (figure moved, dice rolled, wounds applied, figure destroyed, marker changed, token placed, token revealed, phase entered, phase exited, relic claimed, var set, control changed, custom), each with a `Visibility` tag.
- `Decision` kinds (choose figure, choose space, choose option, yes/no, number, order) and `Answer`.
- `GameState::apply(&Event)`: the only mutator, exhaustive over core events.
- `#[derive(StableHash)]` on all state; `GameState::hash()`.

## Acceptance criteria

- [ ] Applying an event never panics on a state where the event is valid; invalid events return an error.
- [ ] Cloning a state and applying events to the clone leaves the original untouched.

## Tests

- One test per event kind for `apply`; hash changes when and only when state changes.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
