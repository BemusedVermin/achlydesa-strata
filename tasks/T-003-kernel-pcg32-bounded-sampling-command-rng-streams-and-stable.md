# T-003: Kernel: PCG32, bounded sampling, command RNG streams and stable hashing

**Task T-003** · M0 Foundations · Est. core lines: ~300 / 500
**Depends on:** T-001

## Goal

Own every bit of randomness and hashing so no dependency bump can ever change a replay.

## Design references

- `docs/design/strata-engine.md` §5.4 Randomness
- `docs/design/strata-engine.md` §5.5 Hashing, snapshots and undo

## Scope

- Our own PCG32 (PCG-XSH-RR, 64-bit state, 32-bit output) with seed and stream selection.
- `bounded(n)` using Lemire's nearly-divisionless method with rejection; `roll(sides)` returning 1..=sides.
- `CommandRng::for_command(game_seed: u64, command_index: u32)` deriving the PCG seed and stream from an xxh3 hash of both values (document the exact derivation).
- `StableHash` trait and `StableHasher` (xxh3-64 via `xxhash-rust`) with implementations for integers (little-endian), `bool`, `str`/`String` (length-prefixed), `Option`, `Vec`, slices, tuples up to 4, `BTreeMap`, `BTreeSet`, `IndexMap` (insertion order).

## Acceptance criteria

- [ ] PCG32 output matches the reference implementation's published test vectors; cite the source in the test.
- [ ] `bounded` never returns a value ≥ n and has no modulo bias.
- [ ] Hashing is independent of platform endianness and pointer width.

## Tests

- Known-answer test for PCG32.
- A deterministic chi-square sanity check of `bounded(6)` over a fixed seed.
- Golden-value tests for `StableHash` of a few fixed structures (constants committed).

## Out of scope

- The derive macro (T-004).

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
