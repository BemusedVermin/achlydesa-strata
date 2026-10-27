# T-023: Rules: resolver with answer tape, events, replacements and triggers

**Task T-023** · M2 Rules core · Est. core lines: ~450 / 500
**Depends on:** T-022

## Goal

Resolve a command deterministically, pausing for decisions without ever suspending code.

## Design references

- `docs/design/strata-engine.md` §5.1 Commands, events, decisions
- `docs/design/strata-engine.md` §5.2 Resolution with an answer tape
- `docs/design/strata-engine.md` §5.3 Pipeline
- `docs/design/strata-engine.md` §6.7 Triggers, replacements and windows

## Scope

- `Ctx` implementation over a working clone of `GameState`; `Halt::NeedsDecision`; decision cap of 64 per command.
- Emit pipeline: replacements (each at most once per event; affected controller orders several) → `apply` → queue triggers.
- Trigger queue: active player first; a decision orders a player's simultaneous triggers; trigger limits (once per turn, round, game).
- `Engine::submit(command) -> Outcome` and `Engine::answer(answer) -> Outcome`: re-run from the start with the extended tape; commit, record `CommandRecord`, hash.
- `Engine::legal_commands()` entry point (filled in by T-024).

## Acceptance criteria

- [ ] Re-running a command with the same tape produces identical events and dice.
- [ ] A rejected command leaves state untouched.

## Tests

- Toy handlers in tests: a two-decision command; replacement loop prevention; trigger ordering; cap exceeded.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
