# T-015: Data stage: constructors, the data table and pack-scoped require

**Task T-015** · M1 Lua data stage · Est. core lines: ~450 / 500
**Depends on:** T-014

## Goal

Run the three data stages and collect every definition with its source location.

## Design references

- `docs/design/strata-engine.md` §4.3 Load stages (steps 2–4)
- `docs/design/strata-engine.md` §4.4 IDs
- `docs/design/strata-engine.md` §10.1 Conventions

## Scope

- Run `data.lua`, then `data_updates.lua`, then `data_final.lua` for all packs in load order (later stages optional per pack).
- A constructor registry: each kind (`stat`, `die`, `terrain`, `unit`, ...) registers a Lua global that validates required fields cheaply and records the table into `data[kind][full_id]`, capturing the caller's file and line via `Lua::inspect_stack`.
- Namespaced ID resolution: bare IDs resolve to the current pack, then dependencies in declared order; ambiguity is an error.
- `data.each(kind, filter)` iterating in ID order; raw `pairs(data)` raises an error that points at `data.each`.
- Pack-scoped `require` resolving only within the pack and its declared dependencies, caching and freezing module tables.

## Acceptance criteria

- [ ] Edits made in `data_updates.lua` are visible to later packs and to the freeze.
- [ ] Every record knows the file and line that created it.

## Tests

- Stage ordering; cross-pack edits; ambiguous-ID error; `require` boundary error; source locations.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
