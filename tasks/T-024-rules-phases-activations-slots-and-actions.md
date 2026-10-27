# T-024: Rules: phases, activations, slots and actions

**Task T-024** · M2 Rules core · Est. core lines: ~450 / 500
**Depends on:** T-023

## Goal

Generic game flow that modules fill in.

## Design references

- `docs/design/strata-engine.md` §6.4 Phases
- `docs/design/strata-engine.md` §6.5 Actions
- `docs/design/strata-engine.md` §12.4 Flow and pending decisions

## Scope

- Phase tree with named anchors (`before`, `after`, `inside`), `PhaseEntered`/`PhaseExited` events, `FlowState` stack updates.
- Activation model: steps, per-figure slot budgets, consume and grant slots, ordering policies (`all_figures_per_step`, `per_figure`).
- `ActionDef` execution: targeting validation, slot consumption, then the resolver.
- `legal_commands()` derived from the current phase, activation and actions.

## Acceptance criteria

- [ ] A module inserting a phase at an anchor needs no change to other modules.

## Tests

- A toy ruleset with two phases, one inserted phase, and an activation of move then attack.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
