# T-069: Release packager

**Task T-069** · M7 Release readiness · Est. core lines: ~300
**Depends on:** T-018

## Goal

Produce a distributable build of the client and its public packs.

## Design references

- `docs/design/strata-engine.md` §4.2 Manifest

## Scope

- `strata package --out dist/` bundling the client binary, public packs and their assets.
- Refuse any pack with `visibility = "private"`.
- A manifest listing bundled packs with their content hashes.

## Acceptance criteria

- [ ] Packaging with a private pack present in the search path fails with a clear message.

## Tests

- Fixture pack sets with and without a private pack.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
