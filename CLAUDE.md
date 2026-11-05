# Strata — guide for implementation agents

You are implementing the Strata engine one GitHub issue at a time. A human maintainer (Liam) reviews and merges every pull request. An automated Claude review runs on every PR to `master` before he looks at it.

## Sources of truth

1. **The issue you are working on.** Its scope, acceptance criteria and out-of-scope list are binding.
2. **`docs/design/strata-engine.md`.** The design. Issues cite sections as `§N.N`. If the issue and the design disagree, follow the issue and say so in the PR description.
3. **This file.** Process and engineering rules.

If something is ambiguous, do not guess on design decisions. Comment on the issue with the question and your proposed answer, and stop.

## Workflow for one issue

1. Make sure the issue is not blocked. GitHub tracks this as the issue's **Blocked by** relationships; this must print nothing:
   ```
   gh api repos/{owner}/{repo}/issues/<N>/dependencies/blocked_by --jq '.[] | select(.state == "open") | .number'
   ```
   If it prints issue numbers, stop: the work isn't ready.
2. Read the cited design sections in full.
3. Branch from `master`: `t-NNN-short-slug` (for example `t-014-pack-manifests`).
4. Implement only the issue's scope. Anything else becomes a note in the PR under "Follow-ups", not code.
5. Before opening the PR, run:
   ```
   cargo fmt --all
   cargo clippy -p <each crate the PR changed> --all-targets --locked -- -D warnings
   cargo test --locked                       # core crates (default members): fast
   cargo test -p strata_client -p strata_render -p strata_scene --locked   # only if the PR touches client or render code
   python scripts/line_budget.py --base origin/master
   ```
   **Run these one at a time, never concurrently.** Liam develops on the same laptop, and parallel cargo builds slow it to a crawl. Concretely:
   - Run at most one `cargo` command at a time, and do not start another until the previous one has finished. Never start a second copy of a build or test run that is still going.
   - Pass `-j 2` to every `cargo` command that compiles.
   - Prefer the narrowest check while iterating (`cargo check -p <crate> --locked`, `cargo test -p <crate> --locked`). Run the commands above once, at the end.
   - Do not run `cargo clippy --workspace` locally. It builds Bevy and every other crate, and CI runs it on the whole workspace anyway. Clippy only the crates the PR changed (a crate that depends on a changed one is checked by CI).
   - A command that moves to the background after the tool timeout is still running. Wait for its notification. Stopping the task does not always stop its child processes, so check for stray `cargo`, `rustc` and `clippy-driver` processes and kill them before running anything else.
   - Do not work around a slow build by running more builds. If a check is taking too long, say so and ask.
6. Open a PR to `master` titled `T-NNN: <issue title>`, fill in the PR template, and include `Closes #<issue number>`.
7. Address every comment from the automated review: fix it, or reply explaining why not. Then leave the PR for Liam.

One issue per PR. Never push to `master` directly. Never modify `.github/`, `scripts/line_budget.py` or this file unless the issue says to.

## The 500-line rule

Each PR may add at most **500 lines of core logic**. CI enforces this with `scripts/line_budget.py`.

**Counted** (added lines that are neither blank nor comment-only):
- `crates/*/src/**/*.rs`, except test files (below)
- `crates/*/build.rs`
- Lua logic in packs: `packs/*/rules/`, `packs/*/abilities/`, `packs/*/scripts/`, `packs/*/lib/`

**Not counted:**
- Tests: `crates/*/tests/**`, `src/**/tests.rs`, `src/**/*_tests.rs`, `src/**/tests/**`
- `benches/`, `examples/`, `fixtures/`, `docs/`, `.github/`, `scripts/`, `Cargo.toml`, `Cargo.lock`
- Pack data: `units/`, `terrain/`, `relics/`, `maps/`, `scenarios/`, `formats/`, `assets/`, `assets.lua`, `pack.lua`, `data*.lua`, `packs/*/tests/`

Unit tests go in a separate file: write `#[cfg(test)] mod tests;` in the module and put the tests in `tests.rs` beside it. Inline `mod tests { ... }` blocks inside counted files are rejected by the budget script.

If the work will not fit in 500 lines, **split it**: open the PR for the first coherent part and comment on the issue with the proposed remainder. Never compress code (long lines, dense formatting, removed docs) to fit. The `budget-exempt` label is for Liam only.

## Architecture rules

- **Layering** (§3). A crate depends only on crates that come earlier in this lowest-to-highest order:
  `strata_kernel` → `strata_model` → `strata_rules` → (`strata_modules`, `strata_lua`) → `strata_content` → (`strata_session`, `strata_editor`, `strata_scene`) → (`strata_ai`, `strata_render`) → (`strata_cli`, `strata_client`).
  `strata_derive` is a proc-macro used by kernel and model.
- **Bevy** appears only in `strata_render` and `strata_client`. `strata_scene` is presentation math with no Bevy. **mlua** appears only in `strata_lua`. `cargo deny` enforces both.
- **Events are the only mutator** of `GameState` (§5.1). Rules never write state directly.
- **Lua holds no state.** All game state lives in Rust; scripts use `ctx` (§9).

## Determinism rules (§5.6, §9.4)

Core crates are everything except `strata_client`, `strata_render`, `strata_scene` and `strata_cli`. In core crates:

- No floating point. Each core `lib.rs` has `#![deny(clippy::float_arithmetic)]`. Use integers and the kernel's exact rationals.
- No `HashMap` or `HashSet` (clippy `disallowed-types`). Use `BTreeMap`, `BTreeSet`, `IndexMap` or `Vec`.
- No clocks (`Instant::now`, `SystemTime::now`) and no `rand` crate. All randomness comes from `strata_kernel`'s PCG32 streams.
- `#![forbid(unsafe_code)]`.
- Anything that can affect a replay must be covered by a test that runs it twice and compares hashes.

## Engineering conventions

- Rust edition 2024, toolchain pinned by `rust-toolchain.toml`. Commit `Cargo.lock`.
- Library crates: `#![warn(missing_docs)]`; document every public item with one or two plain sentences.
- Errors: `thiserror` in libraries; `miette` in binaries for user-facing diagnostics.
- Names follow the design doc (`ScriptRef`, `CompiledBoard`, `RuleModule`, `Registrar`, ...).
- Prefer small modules (one concept per file) over large files.
- Integer units: levels for terrain, sub-levels (12 per level) for LOS heights, axial `(q, r)` for hexes.

### Approved dependencies

Use these freely within the layering rules. Anything else must be justified in the PR description.

`serde`, `serde_json` (dev/tests and tooling only), `postcard`, `thiserror`, `miette`, `smallvec`, `indexmap`, `xxhash-rust` (feature `xxh3`), `sha2`, `semver`, `syn`, `quote`, `proc-macro2` (strata_derive only), `mlua` (strata_lua only; features `lua54`, `vendored`, `serde`), `clap` (derive), `crossbeam-channel`, `rayon`, `notify`, `full_moon`, `image` (CLI sprite lint only), `schemars` (stub generation), `bevy` and `bevy_egui` (strata_client and strata_render; `bevy_egui` in strata_client only), and as dev-dependencies `proptest`, `insta`, `assert_cmd`, `tempfile`.

## Tests

- Every acceptance criterion has at least one test.
- Geometry and math: `proptest` invariants (symmetry, rotation invariance, triangle inequality, and so on).
- Rules behavior: Rust integration tests with rigged dice until the pack test harness lands (T-029), then pack tests.
- Snapshot tests (`insta`) for CLI output and event logs; review snapshot changes deliberately.
- Determinism: same seed and commands twice, identical hash sequence.
- Test and fixture content uses short generic names (`test_soldier`, `test_archer`, `grass`, `rock`).

## When the automated review disagrees with you

Fix what is right. For anything you disagree with, reply in the thread with your reasoning and cite the issue or design section. Do not argue in circles; Liam decides.
