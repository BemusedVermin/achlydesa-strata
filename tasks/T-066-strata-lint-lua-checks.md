# T-066: `strata lint`: Lua checks

**Task T-066** · M6 AI and tooling · Est. core lines: ~400 / 500
**Depends on:** T-019

## Goal

Catch nondeterministic or drifting Lua before it runs.

## Design references

- `docs/design/strata-engine.md` §9.4 Determinism rules
- `docs/design/strata-engine.md` §8.4 Card text
- `docs/design/strata-engine.md` §16.1 CLI

## Scope

- Parse pack Lua with `full_moon`.
- Errors: `/` division in logic files; assignments to upvalues inside functions stored in definitions (hooks); `pairs`-dependent side effects are out of scope.
- Warnings: an explicit `text = ...` override that omits a number present in the archetype parameters.
- Run `selene` with a generated Strata std definition if it is installed; skip with a notice otherwise.

## Acceptance criteria

- [ ] Diagnostics name file, line and a fix.

## Tests

- Fixture Lua files for each check; snapshot of output.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
