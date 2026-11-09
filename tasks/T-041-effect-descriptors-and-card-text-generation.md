# T-041: Effect descriptors and card-text generation

**Task T-041** · M3 Standard rules and archetypes · Est. core lines: ~400
**Depends on:** T-025, T-038

## Goal

Finish the descriptor vocabulary so archetypes can be data.

## Design references

- `docs/design/strata-engine.md` §8.2 Descriptor vocabulary
- `docs/design/strata-engine.md` §8.4 Card text

## Scope

- `eff.*`: `wound`, `heal`, `place`, `remove`, `move`, `grant_slot`, `add_marker`, `take_control`, `cancel_wounds`, `excess_blocks_wound_attacker`.
- Remaining selectors and conditions that need modules: `engaged`, `in_los`, `attack_kind`.
- Card-text rendering: template strings with `{param}` substitution and descriptor phrases, producing text like "Adjacent friendly soldier figures add 1 to their defense."

## Acceptance criteria

- [ ] Every descriptor in §8.2 evaluates natively and renders a phrase.

## Tests

- One test per effect; insta snapshots of rendered card text.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
