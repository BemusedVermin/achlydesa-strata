# T-032: core:rounds and core:command_tokens

**Task T-032** · M3 Standard rules and archetypes · Est. core lines: ~450 / 500
**Depends on:** T-031

## Goal

The round loop, initiative and hidden command tokens.

## Design references

- `docs/design/strata-engine.md` §6.1 Slots
- `docs/design/strata-engine.md` §6.3 Rulesets
- `docs/design/strata-engine.md` §12.5 Hidden information

## Scope

- `core:rounds`: rounds, initiative roll with a configurable die and tie rule, turns per round, round end.
- `core:command_tokens`: numbered tokens plus decoys placed secretly on unit cards (owner-only visibility), revealed in order, deciding which unit activates each turn; tokens removed when a unit is destroyed.
- Empty `tokens` slot: players choose units freely.

## Acceptance criteria

- [ ] Opponents' events and views never contain token assignments before reveal.

## Tests

- Pack tests for initiative ties, token order, decoys, and a destroyed unit holding a token.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
