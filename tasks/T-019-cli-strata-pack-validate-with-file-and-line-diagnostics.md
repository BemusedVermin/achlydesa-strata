# T-019: CLI: `strata pack validate` with file-and-line diagnostics

**Task T-019** · M1 Lua data stage · Est. core lines: ~250 / 500
**Depends on:** T-018

## Goal

The first user-facing tool, and the M1 gate.

## Design references

- `docs/design/strata-engine.md` §16.1 CLI

## Scope

- `clap` CLI `strata` with subcommand groups; unimplemented subcommands print a clear message.
- `strata pack validate <paths...>` loads the pack set and prints every `LoadError` with `miette` (file, line, source snippet, field path), then a summary with the pack-set hash.
- Exit code 0 on success, 1 on load errors, 2 on usage errors.

## Acceptance criteria

- [ ] Every load error shown names its file and line (M1 gate).

## Tests

- `assert_cmd` + `insta` snapshots for a valid pack and for a pack with several errors.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
