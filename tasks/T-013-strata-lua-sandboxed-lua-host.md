# T-013: strata_lua: sandboxed Lua host

**Task T-013** · M1 Lua data stage · Est. core lines: ~450 / 500
**Depends on:** T-005

## Goal

A Lua 5.4 host that cannot do anything nondeterministic or unsafe.

## Design references

- `docs/design/strata-engine.md` §9.3 Sandbox
- `docs/design/strata-engine.md` §9.4 Determinism rules

## Scope

- `mlua` with features `lua54`, `vendored`, `serde`. `LuaHost::new(Limits)` creating a state with only base, `string`, `table`, `math` and `utf8`.
- Remove `io`, `os`, `debug`, `package`, `coroutine`, `dofile`, `loadfile`, `load`, `collectgarbage`, `math.random`, `math.randomseed`, `next`; replace `pairs` with a version that iterates keys in sorted order (numbers before strings, each ascending).
- Per-pack environments built from the sandbox globals, with `freeze(table)` making a table read-only (writes raise).
- Deterministic per-call instruction budget via mlua's instruction-count hook, and `set_memory_limit`.
- A `Halt` sentinel userdata; `pcall`/`xpcall` replacements that rethrow it.
- Conversion `lua_to_value` / `value_to_lua` (T-005 `Value`) rejecting floats, NaN and functions with a typed error.

## Acceptance criteria

- [ ] Two states that insert the same keys in different orders iterate them identically with `pairs`.
- [ ] An infinite loop aborts after the same instruction count every run.
- [ ] A script cannot swallow `Halt` with `pcall`.

## Tests

- One test per removed library and function; sorted pairs; budget; memory limit; float rejection; halt rethrow.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
