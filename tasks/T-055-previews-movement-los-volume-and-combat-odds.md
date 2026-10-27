# T-055: Previews: movement, LOS volume and combat odds

**Task T-055** · M4 Playable client · Est. core lines: ~400 / 500
**Depends on:** T-054, T-037

## Goal

Show players exactly what the rules will do.

## Design references

- `docs/design/strata-engine.md` §13.6 Previews
- `docs/design/strata-engine.md` §7.6 Line of sight with sprites

## Scope

- Movement: shade `reachable` spaces, label costs, flag `stops_here`, `provokes_reprisal` and `falls`.
- Attack: outline the defender's LOS volume as a faint prism around its sprite, draw the witness rays, show clear fraction.
- Exact wound distribution by convolving the two dice definitions (no sampling).

## Acceptance criteria

- [ ] The preview's wound distribution sums to exactly 1 (as rationals).

## Tests

- Unit tests for the convolution against hand-computed distributions.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
