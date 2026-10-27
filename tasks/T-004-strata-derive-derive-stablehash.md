# T-004: strata_derive: #[derive(StableHash)]

**Task T-004** · M0 Foundations · Est. core lines: ~200 / 500
**Depends on:** T-003

## Goal

Make hashing a derive so new state types can't forget a field.

## Design references

- `docs/design/strata-engine.md` §5.5 Hashing, snapshots and undo

## Scope

- Proc-macro crate `strata_derive` (`syn`, `quote`, `proc-macro2`) exporting `#[derive(StableHash)]`, re-exported from `strata_kernel`.
- Structs hash fields in declaration order; enums hash the variant index as `u32` and then the variant's fields; tuple and unit structs supported; simple generics supported with `StableHash` bounds.
- `#[stable_hash(skip)]` attribute for caches that must not affect the hash.

## Acceptance criteria

- [ ] Derived hashes equal hand-written hashes for the same field order.
- [ ] Reordering fields or adding a variant changes the hash (documented behavior).

## Tests

- Unit tests on structs, enums, tuple structs, generics and `skip`, plus golden values.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
