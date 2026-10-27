# T-058: Editor validation and fairness analysis

**Task T-058** · M5 Map editor · Est. core lines: ~400 / 500
**Depends on:** T-057, T-034

## Goal

Tell the map author what is wrong and how fair the map is.

## Design references

- `docs/design/strata-engine.md` §14.1 Editor core

## Scope

- Findings with severity and cells: overlaps, unsupported cells, inventory overruns, zones on unstandable columns, start zones too small for a given army size, relic spots unreachable from start zones.
- Reachability between start zones for a reference one-space figure; per-objective path-length comparison across start zones.
- `strata map check <file>` CLI using the same code.

## Acceptance criteria

- [ ] Validation of an unchanged document is cached and cheap.

## Tests

- Fixture maps for each finding; fairness report snapshot.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
