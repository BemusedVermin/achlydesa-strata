# T-067: Sprite lint: sprite height against the LOS profile

**Task T-067** · M6 AI and tooling · Est. core lines: ~250 / 500
**Depends on:** T-066, T-050

## Goal

Guarantee the picture matches the rules.

## Design references

- `docs/design/strata-engine.md` §7.6 Line of sight with sprites

## Scope

- For every figure with a sprite, load each direction's frames with `image`, measure the opaque bounding box from the foot anchor, convert with `pixels_per_level`, and compare against the hit zone.
- Fail if the height differs by more than one sub-level unit; fail if the width exceeds the footprint's projected width.

## Acceptance criteria

- [ ] Generated fallback sprites always pass.

## Tests

- Fixture PNGs that pass, are too tall, and are too wide.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
