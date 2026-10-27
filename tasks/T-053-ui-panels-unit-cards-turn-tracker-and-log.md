# T-053: UI panels: unit cards, turn tracker and log

**Task T-053** · M4 Playable client · Est. core lines: ~450 / 500
**Depends on:** T-047

## Goal

The information a player needs, from queries rather than raw definitions.

## Design references

- `docs/design/strata-engine.md` §13.3 Plugins (UiPlugin)
- `docs/design/strata-engine.md` §8.4 Card text

## Scope

- `bevy_egui` unit cards showing queried (buffed) stats, generated card text, figures alive and wounds.
- Round, turn, active player and token status (own tokens visible, opponents' hidden).
- Scrollable game log from filtered events.

## Acceptance criteria

- [ ] A stat modified by an aura shows its modified value.

## Tests

- Unit tests for the card view-model built from a `PlayerView`.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
