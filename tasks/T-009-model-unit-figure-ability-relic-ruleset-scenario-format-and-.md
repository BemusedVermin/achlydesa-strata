# T-009: Model: unit, figure, ability, relic, ruleset, scenario, format and army definition types

**Task T-009** · M0 Foundations · Est. core lines: ~400 / 500
**Depends on:** T-008

## Goal

The frozen Rust shapes of everything else a pack defines.

## Design references

- `docs/design/strata-engine.md` §10.4 Footprints and units
- `docs/design/strata-engine.md` §10.5 Abilities, frozen
- `docs/design/strata-engine.md` §10.6 Relics
- `docs/design/strata-engine.md` §6.3 Rulesets
- `docs/design/strata-engine.md` §11.2–11.4

## Scope

- `UnitDef`, `FigureDef`, `LosProfile { target, hit_zone: (bottom, top), samples }`, `CardInfo`.
- `AbilityDef` with `Scope`, `ArchetypeInfo`, `ModifierDef`, `ActionDef`, `TriggerDef`, `ReplacementDef`, `ReactionDef`, `vars`; `Logic { Native(Descriptor), Script(ScriptRef) }`; `Descriptor` as a tagged `Value` tree for now (typed in T-025); `ScriptRef { pack, def, path }`.
- `RelicDef`, `RulesetDef` (stats, dice, slot map with `ModuleRef { id, config }` or empty, addons, `extends`, undo policy), `RuleModuleDef` (for Lua modules), `ScenarioDef`, `FormatDef`, `ArmyList`, `TestDef` placeholder.

## Acceptance criteria

- [ ] Field names and units match the design; sub-level units for LOS profiles.

## Tests

- serde round-trip fixtures for each type.

## Definition of done

- [ ] CI green: fmt, clippy, tests on Linux/macOS/Windows, cargo-deny, line budget
- [ ] Core logic ≤ 500 added lines (`scripts/line_budget.py`); if it will not fit, split the work and say so on this issue
- [ ] Every acceptance criterion has a test
- [ ] Automated Claude review addressed; the PR body says `Closes #<this issue>`

_Implementing agent: read `CLAUDE.md` first._
