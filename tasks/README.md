# Engine tasks

Generated from `tasks.yaml` by `scripts/create_issues.py --render-only`.


## M0 Foundations

Workspace, kernel geometry and RNG, model types, determinism lints. Gate: geometry property tests pass; identical golden hashes on Linux, macOS and Windows.

- [T-001](T-001-workspace-skeleton-toolchain-and-determinism-lints.md) Workspace skeleton, toolchain and determinism lints · ~120 lines · depends on —
- [T-002](T-002-kernel-hex-coordinates-distances-rotations-and-footprints.md) Kernel: hex coordinates, distances, rotations and footprints · ~300 lines · depends on T-001
- [T-003](T-003-kernel-pcg32-bounded-sampling-command-rng-streams-and-stable.md) Kernel: PCG32, bounded sampling, command RNG streams and stable hashing · ~300 lines · depends on T-001
- [T-004](T-004-strata-derive-derive-stablehash.md) strata_derive: #[derive(StableHash)] · ~200 lines · depends on T-003
- [T-005](T-005-kernel-symbols-definition-ids-and-the-value-type.md) Kernel: symbols, definition IDs and the Value type · ~250 lines · depends on T-003, T-004
- [T-006](T-006-kernel-exact-2d-segment-traversal-through-the-hex-grid.md) Kernel: exact 2D segment traversal through the hex grid · ~450 lines · depends on T-002
- [T-007](T-007-kernel-3d-occlusion-test-for-columns-and-figure-volumes.md) Kernel: 3D occlusion test for columns and figure volumes · ~350 lines · depends on T-006
- [T-008](T-008-model-vocabulary-terrain-and-board-definition-types.md) Model: vocabulary, terrain and board definition types · ~300 lines · depends on T-002, T-005
- [T-009](T-009-model-unit-figure-ability-relic-ruleset-scenario-format-and-.md) Model: unit, figure, ability, relic, ruleset, scenario, format and army definition types · ~400 lines · depends on T-008
- [T-010](T-010-model-runtime-state-events-commands-and-decisions.md) Model: runtime state, events, commands and decisions · ~450 lines · depends on T-009
- [T-011](T-011-model-board-compilation-from-placed-pieces.md) Model: board compilation from placed pieces · ~400 lines · depends on T-008
- [T-012](T-012-golden-determinism-fixtures-across-platforms.md) Golden determinism fixtures across platforms · ~60 lines · depends on T-007, T-010, T-011

## M1 Lua data stage

mlua sandbox, pack resolution, data stages, freeze, registry, stubs, `strata pack validate`. Gate: every load error names its file and line.

- [T-013](T-013-strata-lua-sandboxed-lua-host.md) strata_lua: sandboxed Lua host · ~450 lines · depends on T-005
- [T-014](T-014-pack-discovery-manifests-and-dependency-resolution.md) Pack discovery, manifests and dependency resolution · ~350 lines · depends on T-013
- [T-015](T-015-data-stage-constructors-the-data-table-and-pack-scoped-requi.md) Data stage: constructors, the data table and pack-scoped require · ~450 lines · depends on T-014
- [T-016](T-016-freeze-convert-records-to-typed-definitions-and-extract-scri.md) Freeze: convert records to typed definitions and extract ScriptRefs · ~350 lines · depends on T-015, T-009
- [T-017](T-017-reference-resolution-and-the-contentregistry.md) Reference resolution and the ContentRegistry · ~350 lines · depends on T-016
- [T-018](T-018-content-hash-and-pack-set-identity.md) Content hash and pack-set identity · ~150 lines · depends on T-017
- [T-019](T-019-cli-strata-pack-validate-with-file-and-line-diagnostics.md) CLI: `strata pack validate` with file-and-line diagnostics · ~250 lines · depends on T-018
- [T-020](T-020-luals-type-stubs-strata-stubs.md) LuaLS type stubs: `strata stubs` · ~400 lines · depends on T-017

## M2 Rules core

Registrar, queries, resolver with answer tape, phases, descriptors, windows, Lua bridge, pack tests. Gate: pack tests pass with one slot swapped between Rust and Lua.

- [T-021](T-021-rules-rulemodule-trait-registrar-and-ruleset-resolution.md) Rules: RuleModule trait, Registrar and ruleset resolution · ~450 lines · depends on T-017, T-010
- [T-022](T-022-rules-queries-modifiers-and-memoization.md) Rules: queries, modifiers and memoization · ~400 lines · depends on T-021
- [T-023](T-023-rules-resolver-with-answer-tape-events-replacements-and-trig.md) Rules: resolver with answer tape, events, replacements and triggers · ~450 lines · depends on T-022
- [T-024](T-024-rules-phases-activations-slots-and-actions.md) Rules: phases, activations, slots and actions · ~450 lines · depends on T-023
- [T-025](T-025-rules-native-descriptor-evaluator-selectors-conditions-value.md) Rules: native descriptor evaluator (selectors, conditions, values) · ~450 lines · depends on T-022, T-011
- [T-026](T-026-rules-windows-and-reactions.md) Rules: windows and reactions · ~250 lines · depends on T-023
- [T-027](T-027-lua-runtime-states-and-scriptref-calls.md) Lua runtime states and ScriptRef calls · ~400 lines · depends on T-023, T-017
- [T-028](T-028-lua-ctx-api-and-lua-rule-modules.md) Lua ctx API and Lua rule modules · ~450 lines · depends on T-027, T-024, T-025
- [T-029](T-029-pack-test-harness-and-strata-pack-test.md) Pack test harness and `strata pack test` · ~450 lines · depends on T-028
- [T-030](T-030-toy-ruleset-with-a-slot-swapped-between-rust-and-lua-m2-gate.md) Toy ruleset with a slot swapped between Rust and Lua (M2 gate) · ~200 lines · depends on T-029, T-026

## M3 Standard rules and archetypes

Rust modules for every slot, the eight archetype families, strata_std content, session, determinism harness. Gate: 1,000 self-play games with zero hash mismatches.

- [T-031](T-031-core-units-and-dice-rolling.md) core:units and dice rolling · ~350 lines · depends on T-030
- [T-032](T-032-core-rounds-and-core-command-tokens.md) core:rounds and core:command_tokens · ~450 lines · depends on T-031
- [T-033](T-033-core-activation.md) core:activation · ~300 lines · depends on T-032
- [T-034](T-034-core-movement-reachable-for-one-space-figures.md) core:movement: reachable() for one-space figures · ~450 lines · depends on T-033, T-011
- [T-035](T-035-core-movement-multi-space-footprints-and-falling.md) core:movement: multi-space footprints and falling · ~400 lines · depends on T-034
- [T-036](T-036-core-engagement-and-reprisals.md) core:engagement and reprisals · ~250 lines · depends on T-034
- [T-037](T-037-core-los-geometric-policy-with-sprite-aware-volumes.md) core:los: geometric policy with sprite-aware volumes · ~400 lines · depends on T-007, T-031
- [T-038](T-038-core-combat.md) core:combat · ~400 lines · depends on T-036, T-037
- [T-039](T-039-core-relics-and-core-deployment.md) core:relics and core:deployment · ~400 lines · depends on T-033
- [T-040](T-040-core-army-and-core-victory.md) core:army and core:victory · ~350 lines · depends on T-038
- [T-041](T-041-effect-descriptors-and-card-text-generation.md) Effect descriptors and card-text generation · ~400 lines · depends on T-025, T-038
- [T-042](T-042-strata-std-the-eight-archetype-families-and-keywords.md) strata_std: the eight archetype families and keywords · ~450 lines · depends on T-041
- [T-043](T-043-strata-std-base-content-and-eight-test-units.md) strata_std: base content and eight test units · ~150 lines · depends on T-042, T-039, T-040
- [T-044](T-044-strata-session-match-setup-command-loop-saves-and-replays.md) strata_session: match setup, command loop, saves and replays · ~450 lines · depends on T-030
- [T-045](T-045-simulation-thread-channels-and-a-random-controller.md) Simulation thread, channels and a random controller · ~300 lines · depends on T-044
- [T-046](T-046-determinism-harness-and-minimal-strata-sim-m3-gate.md) Determinism harness and minimal `strata sim` (M3 gate) · ~250 lines · depends on T-045, T-043

## M4 Playable client

Bevy 2.5D isometric client with sprite figures, playback, previews, decision UI and hotseat flow. Gate: a full hotseat match with zero art assets.

- [T-047](T-047-client-skeleton-bevy-app-and-simlinkplugin.md) Client skeleton: Bevy app and SimLinkPlugin · ~300 lines · depends on T-045
- [T-048](T-048-isometric-camera-and-kernel-based-picking.md) Isometric camera and kernel-based picking · ~350 lines · depends on T-047
- [T-049](T-049-terrain-rendering.md) Terrain rendering · ~450 lines · depends on T-048
- [T-050](T-050-sprite-figures-billboards-directional-frames-and-fallbacks.md) Sprite figures: billboards, directional frames and fallbacks · ~450 lines · depends on T-049
- [T-051](T-051-occlusion-silhouettes-and-column-fading.md) Occlusion: silhouettes and column fading · ~300 lines · depends on T-050
- [T-052](T-052-event-playback.md) Event playback · ~400 lines · depends on T-050
- [T-053](T-053-ui-panels-unit-cards-turn-tracker-and-log.md) UI panels: unit cards, turn tracker and log · ~450 lines · depends on T-047
- [T-054](T-054-decision-ui-and-command-input.md) Decision UI and command input · ~450 lines · depends on T-053, T-048
- [T-055](T-055-previews-movement-los-volume-and-combat-odds.md) Previews: movement, LOS volume and combat odds · ~400 lines · depends on T-054, T-037
- [T-056](T-056-hotseat-match-flow-m4-gate.md) Hotseat match flow (M4 gate) · ~350 lines · depends on T-052, T-054

## M5 Map editor

Editor core, validation, Lua writer, editor UI. Gate: build, validate and playtest a map without leaving the app.

- [T-057](T-057-strata-editor-map-document-and-undoable-edit-commands.md) strata_editor: map document and undoable edit commands · ~400 lines · depends on T-011
- [T-058](T-058-editor-validation-and-fairness-analysis.md) Editor validation and fairness analysis · ~400 lines · depends on T-057, T-034
- [T-059](T-059-lua-map-writer-with-stable-output.md) Lua map writer with stable output · ~250 lines · depends on T-057
- [T-060](T-060-editor-ui-palette-placement-and-brushes.md) Editor UI: palette, placement and brushes · ~450 lines · depends on T-057, T-049
- [T-061](T-061-editor-ui-slicing-zones-relics-symmetry-validation-playtest-.md) Editor UI: slicing, zones, relics, symmetry, validation, playtest (M5 gate) · ~450 lines · depends on T-060, T-058, T-059

## M6 AI and tooling

Greedy AI, balance sim, coverage stats, rules inspector, hot reload, lints, replay tools. Gate: the AI beats a random player in 90% of games.

- [T-062](T-062-greedy-ai-controller-and-evaluator-terms.md) Greedy AI controller and evaluator terms · ~450 lines · depends on T-046
- [T-063](T-063-strata-sim-balance-stats-ability-coverage-and-slow-path-prof.md) `strata sim` balance stats, ability coverage and slow-path profiler · ~350 lines · depends on T-062
- [T-064](T-064-rules-inspector.md) Rules inspector · ~300 lines · depends on T-022, T-053
- [T-065](T-065-hot-reload.md) Hot reload · ~350 lines · depends on T-056
- [T-066](T-066-strata-lint-lua-checks.md) `strata lint`: Lua checks · ~400 lines · depends on T-019
- [T-067](T-067-sprite-lint-sprite-height-against-the-los-profile.md) Sprite lint: sprite height against the LOS profile · ~250 lines · depends on T-066, T-050
- [T-068](T-068-strata-replay-verify-dump-and-to-test.md) `strata replay verify`, `dump` and `to-test` · ~300 lines · depends on T-044, T-019

## M7 Release readiness

Release packaging. Gate: a distributable build.

- [T-069](T-069-release-packager.md) Release packager · ~300 lines · depends on T-018

## Post-1.0

Work after the engine ships.

- [T-070](T-070-campaign-mode-kalpa.md) Campaign mode (Kalpa) · ~0 lines · depends on T-056
