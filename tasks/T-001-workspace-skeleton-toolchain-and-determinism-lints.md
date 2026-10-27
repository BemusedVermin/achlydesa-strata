# T-001: Workspace skeleton, toolchain and determinism lints

**Task T-001** · M0 Foundations · Est. core lines: ~120 / 500
**Depends on:** nothing

## Goal

Create the Cargo workspace and every guard rail the later tasks rely on, so CI is meaningful from the first real PR.

## Design references

- `docs/design/strata-engine.md` §3 Architecture
- `docs/design/strata-engine.md` §5.6 Determinism guards

## Scope

- Cargo workspace (edition 2024, resolver 3) with library crates `strata_kernel`, `strata_model`, `strata_rules`, `strata_modules`, `strata_lua`, `strata_content`, `strata_session`, `strata_ai`, `strata_editor`, proc-macro crate `strata_derive`, binary crate `strata_cli` (binary name `strata`) and binary crate `strata_client` (binary name `strata-client`, empty `main` for now; Bevy arrives in T-047).
- Each crate depends only on the crates CLAUDE.md's layering allows; add the path dependencies now so the graph is fixed.
- `rust-toolchain.toml` pinning the current stable release (1.85 or later) with `rustfmt` and `clippy` components; `rustfmt.toml` with `edition = "2024"`.
- Every core crate's `lib.rs` (all but `strata_cli` and `strata_client`) starts with `#![forbid(unsafe_code)]`, `#![deny(clippy::float_arithmetic)]` and `#![warn(missing_docs)]`.
- `clippy.toml`: `disallowed-types` for `std::collections::HashMap` and `std::collections::HashSet`; `disallowed-methods` for `std::time::Instant::now` and `std::time::SystemTime::now`.
- `deny.toml`: permissive license allowlist (MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0); `[bans]` entries using `wrappers` so `bevy` may only be a dependency of `strata_client` and `mlua` only of `strata_lua`.
- `.gitignore` including `target/`; commit `Cargo.lock`; add a short Building section to README.md.

## Acceptance criteria

- [ ] `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `cargo deny check` pass on CI for Linux, macOS and Windows.
- [ ] Adding a `HashMap` to a core crate locally makes clippy fail (state in the PR that you verified this; do not commit it).
- [ ] The crate dependency graph matches the layering in CLAUDE.md.

## Tests

- CI green is the test; no unit tests required.

## Out of scope

- Any real logic.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
