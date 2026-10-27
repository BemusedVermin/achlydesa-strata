# T-018: Content hash and pack-set identity

**Task T-018** · M1 Lua data stage · Est. core lines: ~150 / 500
**Depends on:** T-017

## Goal

Give every loaded pack set a fingerprint that replays can verify.

## Design references

- `docs/design/strata-engine.md` §4.3 Load stages (step 6)
- `docs/design/strata-engine.md` §4.5 Versioning and identity

## Scope

- SHA-256 over the canonical `postcard` serialization of the registry plus every pack script's bytes, ordered by path.
- `PackIdentity { id, version, content_hash }` per pack and for the whole set; exposed on the registry.

## Acceptance criteria

- [ ] Changing any definition or any script byte changes the hash; reloading unchanged packs does not.

## Tests

- The three cases above with fixture packs copied to a temp dir.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
