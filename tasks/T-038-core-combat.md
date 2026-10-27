# T-038: core:combat

**Task T-038** · M3 Standard rules and archetypes · Est. core lines: ~400 / 500
**Depends on:** T-036, T-037

## Goal

The standard attack.

## Design references

- `docs/design/strata-engine.md` §6.1 Slots
- `docs/design/strata-engine.md` §6.7 Triggers, replacements and windows
- `docs/design/strata-engine.md` §7.6 Line of sight with sprites

## Scope

- The `attack` action: range from the `range` stat and hex distance, LOS required beyond adjacency, attack and defense pools from queries, height bonus from config, hits minus blocks as wounds.
- Windows `combat.before_attack_roll`, `combat.before_defense_roll`, `combat.after_defense_roll`, `combat.after_wounds`.
- Optional `cover` config: +1 defense when at least half the LOS samples are blocked.
- Lua namespace `ctx.combat.attack { ... }` for special attacks.

## Acceptance criteria

- [ ] All randomness comes from the command RNG; rigged dice in tests fully determine outcomes.

## Tests

- Pack tests for range, height bonus, cover, and a reaction in each window.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
