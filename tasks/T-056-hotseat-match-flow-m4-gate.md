# T-056: Hotseat match flow (M4 gate)

**Task T-056** · M4 Playable client · Est. core lines: ~350 / 500
**Depends on:** T-052, T-054

## Goal

Two people can play a full match on one machine with zero art.

## Design references

- `docs/design/strata-engine.md` §17 Build order (M4 gate)

## Scope

- Main menu: pick packs, ruleset, scenario and army lists (from files).
- Hidden information handoff: a pass-the-device screen before each player's secret token placement.
- Victory screen with a summary and save-replay button.

## Acceptance criteria

- [ ] A full match of the `strata_std` scenario plays to a victory using only fallback visuals.

## Tests

- A headless smoke test driving a match with scripted inputs through the UI layer.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
