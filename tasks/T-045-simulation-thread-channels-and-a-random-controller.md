# T-045: Simulation thread, channels and a random controller

**Task T-045** · M3 Standard rules and archetypes · Est. core lines: ~300 / 500
**Depends on:** T-044

## Goal

Run the session on its own thread so the client never touches Lua or blocks.

## Design references

- `docs/design/strata-engine.md` §13.1 The simulation thread

## Scope

- `SimHandle`: spawns the session thread with its own runtime Lua state; `crossbeam-channel` in (commands, answers, control) and out (filtered event batches, player views, errors).
- `HumanController` fed from the inbound channel.
- `RandomController` choosing uniformly among legal commands and answers using its own seeded kernel RNG.

## Acceptance criteria

- [ ] Shutting down the handle joins the thread cleanly.

## Tests

- Two random controllers play a match through the channels to completion.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
