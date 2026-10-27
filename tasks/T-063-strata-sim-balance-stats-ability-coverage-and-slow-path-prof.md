# T-063: `strata sim` balance stats, ability coverage and slow-path profiler

**Task T-063** · M6 AI and tooling · Est. core lines: ~350 / 500
**Depends on:** T-062

## Goal

Numbers for balancing and for the M6 coverage gate.

## Design references

- `docs/design/strata-engine.md` §8.6 Measuring coverage
- `docs/design/strata-engine.md` §16.1 CLI

## Scope

- `strata sim` with AI controllers: win rates, points destroyed, per-unit survival, as a table and CSV.
- `strata stats abilities <pack>`: counts of pure-archetype, archetype-plus-Lua and fully custom abilities.
- Dev-build counters of `Logic::Script` calls per ability during sims.

## Acceptance criteria

- [ ] Coverage counts match a hand-counted fixture pack.

## Tests

- Snapshot tests of the reports on fixture packs.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
