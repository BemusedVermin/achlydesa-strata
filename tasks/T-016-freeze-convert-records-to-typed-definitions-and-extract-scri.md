# T-016: Freeze: convert records to typed definitions and extract ScriptRefs

**Task T-016** · M1 Lua data stage · Est. core lines: ~350 / 500
**Depends on:** T-015, T-009

## Goal

Turn Lua tables into the typed structs from T-008 and T-009, without losing where anything came from.

## Design references

- `docs/design/strata-engine.md` §4.3 Load stages (step 5)
- `docs/design/strata-engine.md` §9.1 Loader and runtime states

## Scope

- Walk each record; replace every Lua function with `ScriptRef { pack, def, path }`, where `path` is a stable `/`-separated route through the record (for example `unit:ironclad_line/abilities/2/after/figure_destroyed`). Document the path grammar.
- Deserialize the function-free table into the typed struct via mlua's serde support.
- Map serde errors to `LoadError { file, line, field_path, message }`.

## Acceptance criteria

- [ ] Every error names file, line and field path.
- [ ] ScriptRef paths are identical across runs and machines.

## Tests

- A fixture pack with one error of each kind (missing field, wrong type, float, unknown field) asserting the exact error text with insta.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
