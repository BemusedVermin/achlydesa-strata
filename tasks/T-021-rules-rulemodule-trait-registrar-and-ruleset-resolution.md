# T-021: Rules: RuleModule trait, Registrar and ruleset resolution

**Task T-021** · M2 Rules core · Est. core lines: ~450 / 500
**Depends on:** T-017, T-010

## Goal

Assemble a ruleset's modules into one immutable rules runtime.

## Design references

- `docs/design/strata-engine.md` §6.1 Slots
- `docs/design/strata-engine.md` §6.2 The module contract
- `docs/design/strata-engine.md` §6.3 Rulesets

## Scope

- `RuleModule` trait and `Registrar` exactly as in §6.2 (phases, command handlers, actions, modifiers, triggers, replacements, windows, services, Lua API descriptors).
- The twelve standard slot names from §6.1 as constants.
- `ModuleCatalog`: built-in Rust modules registered by ID; Lua modules added later by T-028.
- Ruleset resolution: follow `extends`, apply slot overrides including `false`, add addons, look up modules, check dependencies (a hard dependency on an empty slot is an error), validate each config `Value` against the module's `ConfigSchema`.
- `RulesRuntime`: the finished, immutable, `Send + Sync` registration set.

## Acceptance criteria

- [ ] Resolution errors name the ruleset, slot and module.

## Tests

- Fake modules in tests covering extends chains, `false` slots, addons, missing modules, dependency errors and config validation.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
