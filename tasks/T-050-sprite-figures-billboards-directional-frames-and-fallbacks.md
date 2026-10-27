# T-050: Sprite figures: billboards, directional frames and fallbacks

**Task T-050** · M4 Playable client · Est. core lines: ~450 / 500
**Depends on:** T-049

## Goal

Every figure is an upright, camera-facing sprite that sorts correctly against terrain.

## Design references

- `docs/design/strata-engine.md` §13.8 Isometric rendering and sprites
- `docs/design/strata-engine.md` §13.4 Asset fallbacks
- `docs/design/strata-engine.md` §7.6 Line of sight with sprites

## Scope

- `sprite {}` metadata in `assets.lua` (sheet, frame size, foot anchor, six directions, animations); pack-level `pixels_per_level` and `hex_width_px`.
- Upright billboard quads that yaw with the camera, alpha-cutout materials so the depth buffer sorts them; anchored at the foot point on the figure's surface; scaled from pixels per level.
- Frame selection: direction = (figure rotation − camera yaw) mod 6; animation playback with `idle` fallback.
- Generated fallback sprites: a faction-colored capsule at the figure's hit-zone height in six directions, with a footprint-shaped base.

## Acceptance criteria

- [ ] A fallback sprite's height on screen equals its hit-zone height in levels.

## Tests

- Unit tests for direction selection and pixel-to-world scaling.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
