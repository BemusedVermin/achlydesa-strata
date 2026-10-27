# T-027: Lua runtime states and ScriptRef calls

**Task T-027** · M2 Rules core · Est. core lines: ~400 / 500
**Depends on:** T-023, T-017

## Goal

Bind the registry's ScriptRefs to real functions in each thread's own Lua state.

## Design references

- `docs/design/strata-engine.md` §9.1 Loader and runtime states

## Scope

- `RuntimeLua::new(registry, packs)`: create a sandboxed state, run the pack scripts in runtime mode, harvest functions by `ScriptRef` path, and reject the state if the harvested path set's hash differs from the registry's.
- `ScriptCaller` implementation: call a ScriptRef with converted arguments under the instruction budget; propagate `Halt`; convert results with float rejection.
- Runtime states are created per thread and never shared.

## Acceptance criteria

- [ ] Two runtime states built from the same packs call identical functions for every ScriptRef.

## Tests

- Harvest-hash mismatch; budget abort; halt propagation from Lua through Rust.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
