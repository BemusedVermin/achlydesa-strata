# T-051: Occlusion: silhouettes and column fading

**Task T-051** · M4 Playable client · Est. core lines: ~300 / 500
**Depends on:** T-050

## Goal

Players can always see their figures.

## Design references

- `docs/design/strata-engine.md` §13.3 Plugins (OcclusionPlugin)

## Scope

- Draw an outline silhouette of any figure hidden behind terrain (second pass ignoring depth, or a stencil approach).
- Fade terrain columns between the camera and the selected or hovered figure.

## Acceptance criteria

- [ ] A figure fully behind a wall shows its silhouette in its controller's color.

## Tests

- Unit test for the which-columns-to-fade computation.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
