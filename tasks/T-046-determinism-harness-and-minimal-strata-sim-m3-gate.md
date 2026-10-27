# T-046: Determinism harness and minimal `strata sim` (M3 gate)

**Task T-046** · M3 Standard rules and archetypes · Est. core lines: ~250 / 500
**Depends on:** T-045, T-043

## Goal

Prove 1,000 seeded matches produce identical hash sequences across processes and platforms.

## Design references

- `docs/design/strata-engine.md` §16.5 Determinism harness
- `docs/design/strata-engine.md` §17 Build order (M3 gate)

## Scope

- `strata sim --pack ... --format ... --games N --seed S --hashes out.txt` using random controllers, writing one hash-sequence digest per game.
- A `determinism` CI job (Linux, macOS, Windows) that runs the sim twice in separate processes and diffs the outputs, and compares Linux's output against the other platforms'.
- Pack tests also run twice with output comparison.
- This task may edit `.github/workflows/`.

## Acceptance criteria

- [ ] 1,000 games, zero mismatches, on all three platforms.

## Tests

- The CI job is the test.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
