# T-014: Pack discovery, manifests and dependency resolution

**Task T-014** · M1 Lua data stage · Est. core lines: ~350 / 500
**Depends on:** T-013

## Goal

Find packs and decide the order they load in.

## Design references

- `docs/design/strata-engine.md` §4.1 Layout
- `docs/design/strata-engine.md` §4.2 Manifest
- `docs/design/strata-engine.md` §4.3 Load stages (step 1)

## Scope

- Scan one or more pack roots for directories containing `pack.lua`.
- Evaluate `pack.lua` in an empty environment (no globals at all) and convert the returned table into `PackManifest { id, version, name, engine, dependencies, visibility }` using `semver`.
- Resolve: missing dependency, version mismatch, duplicate IDs, cycles, and engine-range mismatch against `strata_content::ENGINE_VERSION` are errors naming the pack and file.
- Topological load order with ties broken by pack ID.

## Acceptance criteria

- [ ] Resolution is a pure function of the manifests (directory listing order does not matter).

## Tests

- Fixture packs under `fixtures/packs/` covering each error and a diamond dependency with tie-breaking.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
