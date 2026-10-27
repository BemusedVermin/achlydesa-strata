# T-020: LuaLS type stubs: `strata stubs`

**Task T-020** · M1 Lua data stage · Est. core lines: ~400 / 500
**Depends on:** T-017

## Goal

Autocomplete and type warnings for pack authors, generated from the same schemas the loader uses.

## Design references

- `docs/design/strata-engine.md` §9.5 Editor support
- `docs/design/strata-engine.md` §16.3 Editor setup

## Scope

- Derive `schemars::JsonSchema` on the definition types (or a small hand-written schema trait if that is cleaner; justify the choice).
- Translate schemas to LuaLS `---@meta` files: one `---@class` per definition, constructor functions with `---@param`, enums as string unions.
- `strata stubs --out <dir>` writes the files; a template `.luarc.json` pointing at them lives in `templates/pack/`.

## Acceptance criteria

- [ ] Stubs regenerate identically from the same source.

## Tests

- insta snapshot of the generated stub for `unit`, `terrain` and `ruleset`.

## Out of scope

- Stubs for `ctx` and archetypes (added by the tasks that create them).

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
