# T-017: Reference resolution and the ContentRegistry

**Task T-017** · M1 Lua data stage · Est. core lines: ~350 / 500
**Depends on:** T-016

## Goal

Resolve every cross-reference and freeze the result into the immutable registry the rules read.

## Design references

- `docs/design/strata-engine.md` §4.4 IDs
- `docs/design/strata-engine.md` §4.3 Load stages (step 5)

## Scope

- Assign `DefId`s per kind in sorted order of the full `pack:name` string.
- Resolve string references (terrain in tiles, footprints in figures, factions, stats, dice, rulesets) to `DefId`s with kind checks; errors name the referencing file and line.
- `ContentRegistry` (behind `Arc`, `Send + Sync`): typed lookup by `DefId` and by full ID, iteration in ID order, and the pack load order.
- `strata_content::load(roots) -> Result<ContentRegistry, Vec<LoadError>>` collecting all errors rather than stopping at the first.

## Acceptance criteria

- [ ] Loading the same packs twice yields equal registries and identical `DefId`s.

## Tests

- Dangling reference, wrong-kind reference, and a full load of a small valid fixture pack.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
