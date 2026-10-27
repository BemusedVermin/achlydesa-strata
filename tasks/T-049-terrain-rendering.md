# T-049: Terrain rendering

**Task T-049** · M4 Playable client · Est. core lines: ~450 / 500
**Depends on:** T-048

## Goal

Draw the compiled board.

## Design references

- `docs/design/strata-engine.md` §13.3 Plugins (BoardPlugin)
- `docs/design/strata-engine.md` §13.8 Isometric rendering and sprites

## Scope

- Build hex-prism meshes per terrain material from `CompiledBoard`: top faces, and side faces only where exposed.
- Chunked meshes for large maps; rebuild only changed chunks on `BoardChanges`.
- Flat-shaded materials with nearest-neighbor textures from `assets.lua`, or tinted fallbacks.
- A level-slicing hook (hide segments above level N) for the editor.

## Acceptance criteria

- [ ] Bridges and overhangs render with their gaps.

## Tests

- Unit tests for mesh face generation (exposed faces only) on fixture columns.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
