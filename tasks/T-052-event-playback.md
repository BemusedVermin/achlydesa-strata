# T-052: Event playback

**Task T-052** · M4 Playable client · Est. core lines: ~400 / 500
**Depends on:** T-050

## Goal

Show what happened, in order, at a readable pace.

## Design references

- `docs/design/strata-engine.md` §13.3 Plugins (PlaybackPlugin)

## Scope

- `EventPlayer` queue with per-event views: figure moves along its path (with `move` animation), dice tumble and result display, wound and marker changes, destruction, token reveal, phase banners.
- Custom events map to a generic view from `assets.lua` (text, optional sound key).
- Input lock during playback; skip and speed controls.

## Acceptance criteria

- [ ] Visual state after playback matches the simulation's state for every figure.

## Tests

- A headless test feeding an event batch and checking final entity transforms.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
