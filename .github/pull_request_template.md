## Task

Closes #<!-- issue number -->

<!-- One or two sentences: what this PR does. -->

## Acceptance criteria

<!-- Copy the checklist from the issue and tick what this PR satisfies. Every ticked item names its test. -->

- [ ] ...

## Notes for the reviewer

<!-- Design decisions, deviations from the issue or design doc (and why), anything fragile. -->

## Follow-ups

<!-- Work you deliberately left out. Each becomes a new issue if accepted. -->

## Checklist

- [ ] `cargo fmt`, `cargo clippy -D warnings` and `cargo test --workspace` pass locally
- [ ] `python3 scripts/line_budget.py --base origin/master` passes (core logic ≤ 500 added lines)
- [ ] Unit tests live in separate `tests.rs` files, not inline modules
- [ ] No floats, `HashMap`/`HashSet`, clocks or `rand` in core crates
- [ ] Public items documented
