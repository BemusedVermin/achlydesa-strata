# T-060: Editor UI: palette, placement and brushes

**Task T-060** · M5 Map editor · Est. core lines: ~450
**Depends on:** T-057, T-049

## Goal

Build terrain by hand, quickly.

## Design references

- `docs/design/strata-engine.md` §14.2 Editor UI

## Scope

- `EditorPlugin` mode switch; palette from loaded packs' tiles and features with inventory counts.
- Placement ghost with rotation, snapping to the top surface under the cursor or a locked level.
- Freeform brushes: single hex, line, region fill; undo and redo bindings.

## Acceptance criteria

- [ ] Every UI action goes through `strata_editor` edit commands.

## Tests

- Unit tests for ghost placement level selection.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
