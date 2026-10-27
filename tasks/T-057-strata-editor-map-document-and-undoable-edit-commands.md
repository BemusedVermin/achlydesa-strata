# T-057: strata_editor: map document and undoable edit commands

**Task T-057** · M5 Map editor · Est. core lines: ~400 / 500
**Depends on:** T-011

## Goal

Editing logic with no Bevy dependency.

## Design references

- `docs/design/strata-engine.md` §14.1 Editor core

## Scope

- `MapDocument` wrapping a `MapDef` plus a compiled board refreshed after each edit.
- Edit commands: place, remove, move and rotate piece; place and remove feature; paint and erase zone cells; set and clear relic spot; set metadata. Each with `apply` and `undo`.
- Undo and redo stacks.

## Acceptance criteria

- [ ] Any sequence of edits followed by the same number of undos restores the original map exactly.

## Tests

- proptest over random edit sequences with undo.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
