# T-005: Kernel: symbols, definition IDs and the Value type

**Task T-005** · M0 Foundations · Est. core lines: ~250 / 500
**Depends on:** T-003, T-004

## Goal

Provide the identifier and dynamic-value types every layer shares.

## Design references

- `docs/design/strata-engine.md` §4.4 IDs
- `docs/design/strata-engine.md` §5.1 Commands, events, decisions

## Scope

- `Sym`: an immutable string compared, ordered and hashed by content (wrap `Arc<str>`; no global interner, so nothing depends on interning order).
- `DefId(u32)` dense handle (assigned later at freeze), `PackId`, `ModuleId`, `SlotId`, `WindowId` as `Sym` newtypes.
- `Value` enum: `Nil`, `Bool`, `Int(i64)`, `Str(Sym)`, `Id(DefId)`, `List(Vec<Value>)`, `Map(BTreeMap<Sym, Value>)`. No float variant. Accessors (`as_int`, `get("key")`, ...) returning `Result` with a typed error.
- serde and `StableHash` for all of the above.

## Acceptance criteria

- [ ] `Value` round-trips through `postcard` and `serde_json` unchanged.
- [ ] `Map` iteration order is key order.

## Tests

- Round-trip tests; ordering tests; golden hash values.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
