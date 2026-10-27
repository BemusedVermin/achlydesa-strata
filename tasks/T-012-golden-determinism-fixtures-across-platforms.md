# T-012: Golden determinism fixtures across platforms

**Task T-012** · M0 Foundations · Est. core lines: ~60 / 500
**Depends on:** T-007, T-010, T-011

## Goal

Prove on all three CI platforms that geometry, RNG and hashing agree bit for bit. This closes the M0 gate.

## Design references

- `docs/design/strata-engine.md` §16.5 Determinism harness
- `docs/design/strata-engine.md` §17 Build order (M0 gate)

## Scope

- `fixtures/boards/` with two small hand-written maps (one with a bridge and a wall).
- A test that compiles them, runs a fixed set of LOS queries, draws 1,000 values from a fixed `CommandRng`, hashes a fixed `GameState`, and compares everything against committed golden values.
- `STRATA_BLESS=1 cargo test` rewrites the goldens; document this in the test file.

## Acceptance criteria

- [ ] The test passes unchanged on Linux, macOS and Windows CI.

## Tests

- The golden test itself.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
