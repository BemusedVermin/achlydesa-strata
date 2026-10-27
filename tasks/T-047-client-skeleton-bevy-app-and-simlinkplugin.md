# T-047: Client skeleton: Bevy app and SimLinkPlugin

**Task T-047** · M4 Playable client · Est. core lines: ~300 / 500
**Depends on:** T-045

## Goal

The Bevy shell that talks to the simulation.

## Design references

- `docs/design/strata-engine.md` §13.1 The simulation thread
- `docs/design/strata-engine.md` §13.3 Plugins
- `docs/design/strata-engine.md` §13.7 Containing Bevy upgrades

## Scope

- Add `bevy` (latest release at the time) and the matching `bevy_egui` to `strata_client` only; pin exact versions.
- `SimLinkPlugin`: starts a `SimHandle` from CLI args (packs, scenario, armies), drains outbound batches into a `PendingEvents` resource each frame, exposes a `SendCommand` event.
- Plugin skeletons for the other plugins in §13.3, empty for now.

## Acceptance criteria

- [ ] `strata-client --pack packs/strata_std --scenario ...` opens a window and logs incoming events.

## Tests

- A headless Bevy `App` test with `MinimalPlugins` verifying the channel drain.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
