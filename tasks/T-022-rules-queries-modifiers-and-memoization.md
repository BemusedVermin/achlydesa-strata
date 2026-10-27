# T-022: Rules: queries, modifiers and memoization

**Task T-022** · M2 Rules core · Est. core lines: ~400 / 500
**Depends on:** T-021

## Goal

Compute every stat and rules question through one composable pipeline.

## Design references

- `docs/design/strata-engine.md` §6.6 Queries, modifiers and descriptors

## Scope

- `Query` trait with typed output; `StatQuery { stat, subject, context }` and boolean vote queries (`can_enter`, `is_engaged`, `can_target`, `provokes_reprisal`, plus custom keys).
- Modifier application in layers `set`, `add`, `mul`, `final`, sorted by (layer, priority, source ID); clamp to `StatDef` bounds; votes resolve deny > allow > default unless `override`.
- A `LogicEvaluator` trait for `Logic::Native` and a `ScriptCaller` trait for `Logic::Script` (implemented in T-025 and T-027).
- Per-resolution-step memo cache, invalidated on every applied event.
- Every query can emit a trace (inputs, each modifier, result) for the inspector in T-064.

## Acceptance criteria

- [ ] Modifier order never depends on registration order.

## Tests

- Layer ordering, clamping, vote precedence, override, cache invalidation.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
