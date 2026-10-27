# T-044: strata_session: match setup, command loop, saves and replays

**Task T-044** · M3 Standard rules and archetypes · Est. core lines: ~450 / 500
**Depends on:** T-030

## Goal

Run whole matches headlessly.

## Design references

- `docs/design/strata-engine.md` §12.2 Match setup
- `docs/design/strata-engine.md` §12.6 Saves and replays
- `docs/design/strata-engine.md` §15.1 Controllers
- `docs/design/strata-engine.md` §15.3 Visibility

## Scope

- `MatchSetup` → initial `GameState` (deploy army lists, place relics, compile board).
- `Controller` trait (§15.1); `Session` loop asking the right controller for commands and answers.
- `PlayerView` projection and per-player event filtering by visibility.
- Replay format (postcard): engine version, pack identities, setup, `CommandRecord`s, optional snapshots every 20 commands; save, load and verify pack hashes before replaying.

## Acceptance criteria

- [ ] Replaying a saved match reproduces every state hash.

## Tests

- Scripted controllers playing a short match on the toy pack; save, load, replay and compare hashes.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
