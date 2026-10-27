# T-028: Lua ctx API and Lua rule modules

**Task T-028** · M2 Rules core · Est. core lines: ~450 / 500
**Depends on:** T-027, T-024, T-025

## Goal

Give scripts the full `ctx` surface and let a Lua module fill any slot.

## Design references

- `docs/design/strata-engine.md` §9.2 API surface
- `docs/design/strata-engine.md` §6.2 The module contract

## Scope

- `ctx.state` read-only proxies (writes raise); `ctx.select`; `ctx.query.*`; `ctx.choose`; `ctx.dice.roll` returning a roll object with `:count`, `:sum`, `:faces`; `ctx.vars(owner)` writing via `VarSet` events; `ctx.emit`; `ctx.log`.
- Module namespaces registered through `Registrar::lua_api` appear as `ctx.<namespace>`.
- `rule_module {}` constructor and an adapter implementing `RuleModule` whose `register` calls the Lua function; its registrations store ScriptRefs.
- Extend `strata stubs` with the `ctx` API.

## Acceptance criteria

- [ ] A Lua module can register an action, a modifier and a trigger, and they behave like their Rust equivalents.

## Tests

- Lua fixture modules exercising every `ctx` entry point.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
