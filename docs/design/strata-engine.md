# Strata — Hex Skirmish Engine Design Document

> Repository copy of the engine design document; implementation tasks reference its sections. Diagrams are summarized in text.

## 1. Purpose, scope and decisions

Strata is a deterministic hex-and-height skirmish engine written in Rust. Every rule, format, unit, ability, map and scenario is Lua content loaded from packs, so new games and variants need no engine changes. This document defines the code structure, the extension model, and the data models for pieces and scenarios.

| Decision | Choice | Why |
| --- | --- | --- |
| Language and engine | Rust core, Bevy client | One language end to end; no GC; strong compile-time tooling for determinism |
| Authoring | Lua for everything, Factorio-style | Stats, card text and behavior live in one file; unique abilities stay inline; a proven model for mods patching each other |
| Abilities | A library of parameterized archetypes, plus inline Lua for unique ones | Most abilities fall into a few general groups; archetypes make them one-liners and keep them fast |
| Core rules | Rust modules in named slots, each replaceable by a Lua module | Native speed and easy debugging by default; any rule is still moddable |
| Lua runtime | `mlua` with Lua 5.4, vendored | Mature bindings, integer subtype, instruction-count hooks |
| Platforms | Desktop: Windows, macOS, Linux | `mlua` does not target Bevy's web build; revisit if web becomes a goal |
| Rendering | 2.5D isometric: an orthographic camera over real 3D hex terrain, with characters drawn as sprites | Sprite art is far cheaper to make than rigged models; 3D terrain keeps stacking, overhangs and camera rotation free |

"Strata" is a working codename.

**Goals**

- Express complete rulesets, formats and rosters as packs, with zero engine changes.
- Let Liam and modders author new rules, formats, units and abilities with the same pipeline the standard pack uses.
- Make most abilities a one-line archetype call, and unique ones a few lines of inline Lua.
- Ship an in-app map editor for stacked-hex terrain.
- Same packs, seed and commands always produce the same game. AI, replays, tests and any future online play all build on that.

**Non-goals for v1**

- Web and mobile builds.
- Online multiplayer. The design keeps it possible (section 15) but does not build it.
- In-app editors for units, abilities or rules. Those are Lua files, backed by type stubs, validation and hot reload (section 16).
- Campaign mode.

## 2. Design principles

Eight rules settle the structural questions in the rest of this document. When a later choice is unclear, the principle wins.

1. **The engine is a rules host, not a game.** The Rust crates know hexes, levels, commands, events, queries, decisions and dice. What "life", "flying" or "water" mean comes from packs.
2. **The standard game is a pack.** `strata_std` loads through the same pipeline as any mod. Any hook it needs, modders get too.
3. **Archetype first, Lua second, Rust last.** An ability is an archetype call when it fits one, inline Lua when it is unique, and a new Rust built-in only when profiling demands it. Every Rust built-in is callable from Lua.
4. **Native by default, replaceable everywhere.** Each core rule is a Rust module in a named slot. A Lua module can take any slot, and only the mod that does so pays the speed cost.
5. **Deterministic by construction.** Integer-only rules math, exact integer geometry, one seeded RNG, ordered collections only. Lints enforce it rather than discipline.
6. **The simulation never touches Bevy.** Rules run on their own thread over plain Rust state. Bevy renders events and sends commands, and the AI clones state freely.
7. **One format.** Even editor-written files (maps, army lists) are Lua table literals, so there is one loader, one validator and one set of tools.
8. **Assets are keyed by content ID.** The engine ships no art of its own; packs bring it, and generated fallbacks fill any gap.

## 3. Architecture

The engine is a Cargo workspace of 11 crates in four layers (plus the `strata_derive` proc-macro helper), and a crate may depend only on crates in layers beneath it. Bevy appears in exactly one crate and `mlua` in exactly one other, so the rules build, run and test headlessly, and either dependency can be swapped by touching one crate.

Layers, top to bottom (diagram summarized):

- **Front ends:** `strata_client` (the only crate that depends on Bevy), `strata_cli`.
- **Application:** `strata_session`, `strata_ai`, `strata_editor`.
- **Rules and scripting:** `strata_rules`, `strata_modules`, `strata_lua` (the only crate that depends on `mlua`), `strata_content`.
- **Foundation:** `strata_model`, `strata_kernel`.
- **Content packs (Lua, not crates):** loaded by `strata_content`. `strata_std` ships with the game; user and modder packs load the same way.

| Crate | Owns | Depends on |
| --- | --- | --- |
| `strata_kernel` | Hex math, exact LOS geometry, PCG32, xxh3 hashing, typed IDs | Nothing in the workspace |
| `strata_model` | Definition structs, `GameState`, events, commands, decisions, `Value` | kernel |
| `strata_rules` | Slots, registrar, phases, actions, query and descriptor evaluator, triggers, replacements, windows, answer-tape resolver | model |
| `strata_modules` | The default Rust module for every slot | rules |
| `strata_lua` | `mlua` host, sandbox, data-stage constructors, Lua module adapter, `ScriptRef` binding, LuaLS stub generator | rules |
| `strata_content` | Pack resolution, stage orchestration, freeze, registry, content hash | lua, model |
| `strata_session` | Match setup, simulation thread, controllers, command loop, saves and replays | content, modules |
| `strata_ai` | Evaluator terms, greedy controller, later MCTS | session |
| `strata_editor` | Map documents, edit commands, validation, fairness analysis, Lua writer | content, modules |
| `strata_cli` | `validate`, `test`, `lint`, `stubs`, `stats`, `sim`, `replay` | Everything except client |
| `strata_client` | Bevy app and all UI | Everything |

`cargo-deny` enforces the two isolation rules with `wrappers`: `bevy` may only be a dependency of `strata_client`, and `mlua` only of `strata_lua`. At runtime the client and the simulation are separate threads joined by channels (section 13.1).

## 4. Content packs and the Lua data stage

A pack is a folder of Lua plus assets. Loading runs each pack's Lua in staged, sandboxed passes that build plain data, then freezes the result into an immutable Rust registry. This is Factorio's model: the base game is just the first mod, and later mods edit earlier ones' definitions with ordinary Lua instead of a patch language.

### 4.1 Layout

```
my_pack/
  pack.lua            manifest (a plain table)
  data.lua            stage 1 entry: define content; requires the files below
  data_updates.lua    stage 2 (optional): edit other packs' definitions
  data_final.lua      stage 3 (optional): fixes that must see every update
  units/  abilities/  rules/  terrain/  relics/
  maps/               editor-written Lua table literals
  scenarios/  formats/
  assets/             sprite sheets, icons, card art, audio
  assets.lua          visual key -> asset path
  tests/              pack tests (section 16)
```

### 4.2 Manifest

```lua
-- pack.lua runs in an empty environment and may only return a table.
return {
  id = "my_pack",
  version = "0.3.0",
  name = "My pack",
  engine = ">=0.1, <0.2",
  dependencies = { strata_std = "^0.1" },
  visibility = "private",
}
```

Private packs are for personal use: they live in the user's data directory, and the release packager never bundles them.

### 4.3 Load stages

1. **Resolve.** Read every manifest, sort packs topologically by dependency (ties broken by ID), and check engine version ranges.
2. **`data`.** Each pack's `data.lua` runs in dependency order. Constructors such as `unit {}` and `terrain {}` check shape immediately and record into a shared `data` table, keyed by kind and ID. Each record remembers its source file and line.
3. **`data_updates`.** Each pack may read and edit any record from any pack.
4. **`data_final`.** The same, for edits that must see everyone else's updates.
5. **Freeze.** Every record converts to a typed Rust struct through `mlua`'s serde support, with errors pointing at the original file and line. References are resolved and kind-checked. Lua functions inside records (hooks, custom predicates) become `ScriptRef { pack, def, path }` handles, bound to real functions per Lua state (section 9.1).
6. **Hash.** A content hash covers the canonical frozen registry plus the bytes of every pack script, since behavior lives in Lua.

A balance mod is then a few lines in `data_updates.lua`:

```lua
for _, u in data.each("unit", { faction = "strata_std:brass_compact" }) do
  u.stats.points = u.stats.points - 5
end
data.unit["strata_std:ironclad_line"].stats.defense = 5
```

`data.each` iterates in ID order. Raw `pairs` over `data` is unavailable, because Lua 5.4 randomizes string hashing per state.

### 4.4 IDs

Every definition has a global ID of the form `pack:name`, scoped by kind: `strata_std:magma` the terrain and `strata_std:magma` the relic can coexist. Inside a pack, a bare `"magma"` resolves to the pack itself, then to its dependencies in declared order. An ambiguous bare reference is a load error. At freeze time, IDs are interned to dense integer handles, assigned in sorted order of the full string so every machine agrees.

### 4.5 Versioning and identity

The constructor functions are the compatibility surface. When a definition's shape changes, the old constructor stays as a converting shim for one major engine version.

Every match records its pack set: ID, version and content hash for each pack. Replays verify those hashes before the first command.

## 5. Core runtime

The core is a pure function from (pack set, seed, setup, command log) to game state. Commands express intent, resolution turns them into events, and events are the only thing that changes state.

### 5.1 Commands, events, decisions

**Commands** are player intents: place tokens, activate a unit, take an action, end an activation. The command log is the save file.

**Events** are state deltas: a figure moved, dice rolled, wounds applied, a figure destroyed, a marker changed, a token revealed, a phase entered, a variable set. `GameState::apply(&Event)` is the only mutator.

**Decisions** are questions the rules ask mid-resolution: pick a target, a space, yes or no, a number, or the order of simultaneous triggers. Each names a chooser, who may not be the active player.

Packs can declare custom commands and events. Custom events carry data for triggers and presentation; actual mutation still goes through core events.

```rust
pub enum Outcome {
    Committed(Vec<Event>),
    NeedsDecision { decision: Decision, chooser: PlayerId },
    Rejected(RuleViolation),
}

pub struct CommandRecord {
    pub index: u32,
    pub player: PlayerId,
    pub command: Command,
    pub answers: Vec<Answer>,   // the answer tape, see 5.2
    pub state_hash: u64,
}

/// What every rule, Rust or Lua, sees while resolving.
pub trait Ctx {
    fn state(&self) -> &GameState;                            // the working copy
    fn rng(&mut self) -> &mut CommandRng;
    fn emit(&mut self, e: Event) -> Result<(), Halt>;         // replacements, apply, queue triggers
    fn choose(&mut self, d: Decision) -> Result<Answer, Halt>;
    fn query<Q: Query>(&mut self, q: Q) -> Q::Output;
}
```

`Value`, the type for module config and custom payloads, is the engine's own enum of integers, booleans, strings, IDs, lists and ordered maps. It has no float variant.

### 5.2 Resolution with an answer tape

Abilities constantly need choices halfway through. A suspended Lua coroutine cannot be serialized, so Strata re-executes instead of suspending.

Resolution runs against a clone of the state. That is cheap, because static board data sits behind an `Arc`. When code calls `choose`, the context returns the next recorded answer from the command's tape. If the tape is exhausted, it returns `Halt::NeedsDecision`, which `?` propagates out of Rust and the Lua bridge propagates out of scripts. Once the chooser answers, the answer joins the tape and the command reruns from the start, with identical dice, because the RNG stream is keyed to the command index.

The cost is rerunning a command once per decision, microseconds for a board game. A cap of 64 decisions per command catches runaway scripts. The payoff is that authors write straight-line code, and a save is just setup plus a list of `CommandRecord`s.

The sandbox's `pcall` rethrows the halt signal, so a script cannot accidentally swallow it.

### 5.3 Pipeline

1. A controller (UI, AI, replay or, later, network) submits a command.
2. Legality runs the same code the UI uses for `legal_commands` and previews.
3. Resolution runs on the working copy with the current tape.
4. Each emitted event passes the replacement hooks, applies to the working copy, then queues its triggers.
5. Queued triggers resolve active player first. A player with several simultaneous triggers is asked to order them.
6. On completion, the working copy replaces the state, the record joins the log, and the state hash is computed.
7. Events go to the client through each player's visibility filter (section 15.3).

### 5.4 Randomness

The kernel crate implements PCG32 and its own bounded sampling (Lemire's method). Nothing depends on the `rand` crate's distribution code, so a dependency bump can never change a replay. Each command's stream is seeded from a hash of the game seed and the command index. Every roll emits a `DiceRolled` event.

### 5.5 Hashing, snapshots and undo

After each command, the core computes a 64-bit xxh3 hash over a `StableHash` traversal of state, with fields in declaration order and maps in key order. Hashes are stored in the log and checked by tests, replays and desync detection.

Snapshots every 20 commands make replay seeking and undo cheap. Rulesets choose the undo policy; the default allows takebacks until a die is rolled or hidden information is revealed.

### 5.6 Determinism guards

Core crates are `#![forbid(unsafe_code)]` and `#![deny(clippy::float_arithmetic)]`. `clippy.toml` disallows `HashMap`, `HashSet`, `Instant::now`, `SystemTime::now` and `rand::thread_rng` in those crates. Collections are `Vec`, `BTreeMap` or `IndexMap`, all of which iterate in a defined order.

## 6. Rules framework

A ruleset fills a fixed set of named slots, each with one module, and then adds any number of add-on modules. Rust provides a default module for every slot, and a Lua module can take any slot. Abilities, terrain and relics then plug into six seams: phases, actions, queries, triggers, replacements and windows.

### 6.1 Slots

| Slot | Responsibility | Default |
| --- | --- | --- |
| `turns` | Round loop, initiative, turn order | `core:rounds` |
| `tokens` | Hidden numbered tokens and decoys; which unit acts when | `core:command_tokens` |
| `activation` | A unit's turn: steps, per-figure slots, ordering | `core:activation` |
| `movement` | Path cost, climbing, stop-on-enter terrain, falling | `core:movement` |
| `engagement` | Who is engaged; reprisals for leaving | `core:engagement` |
| `combat` | Attack and defense pools, hits and blocks, wounds, height bonus | `core:combat` |
| `los` | Line-of-sight service (section 7.5) | `core:los` |
| `relics` | Relic spots, claiming, reveal, duration | `core:relics` |
| `units` | Heroes and squads, wounds, destruction, removal | `core:units` |
| `deployment` | Start zones, placement order, setup steps | `core:deployment` |
| `army` | Army legality: points, uniqueness, copies | `core:army` |
| `victory` | Evaluating victory conditions | `core:victory` |

Add-on modules have no slot. They only add hooks, such as a morale check or a reinforcement phase.

### 6.2 The module contract

```rust
pub trait RuleModule: Send + Sync {
    fn id(&self) -> ModuleId;
    fn slot(&self) -> Option<SlotId>;          // None = add-on
    fn depends_on(&self) -> &[ModuleId];
    fn config_schema(&self) -> &ConfigSchema;
    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError>;
}

impl Registrar {
    pub fn phase(&mut self, p: PhaseDef, at: Anchor);
    pub fn command<H: CommandHandler>(&mut self, h: H);
    pub fn action(&mut self, a: ActionDef);
    pub fn modifier(&mut self, m: ModifierDef);
    pub fn trigger(&mut self, t: TriggerDef);
    pub fn replacement(&mut self, r: ReplacementDef);
    pub fn window(&mut self, w: WindowId);
    pub fn service<S: ?Sized + Service>(&mut self, s: Box<S>);   // e.g. dyn LineOfSight
    pub fn lua_api(&mut self, namespace: &str, api: LuaApi);     // e.g. ctx.combat
}
```

A Lua module implements the same contract. This one replaces combat so the defender rolls first:

```lua
rule_module {
  id = "defender_first",
  slot = "combat",
  depends = { "core:units" },
  config = { min_wounds = { type = "int", default = 0 } },
  register = function(r, cfg)
    r.action {
      id = "attack",
      slots = { "attack" },
      targets = sel.enemies { in_range = true, in_los = true },
      resolve = function(ctx, attacker, target)
        local def = ctx.dice.roll("combat", ctx.query.stat(target, "defense"))
        local atk = ctx.dice.roll("combat", ctx.query.stat(attacker, "attack"))
        local wounds = math.max(cfg.min_wounds, atk:count("hit") - def:count("block"))
        ctx.units.wound(target, wounds, { source = attacker })
      end,
    }
  end,
}
```

`ctx.units` and `ctx.dice` exist because those modules export Lua namespaces through `lua_api`. A replacement module reuses the pieces it does not change.

Registrations from Lua store `ScriptRef` handles, not functions, so the finished registry is plain `Send + Sync` data shared by every thread.

### 6.3 Rulesets

```lua
ruleset {
  id = "advanced",
  stats = { "life", "move", "range", "attack", "defense", "points" },
  dice = { "combat", "d20" },
  slots = {
    turns      = { "core:rounds", turns_per_round = 3, initiative = "d20" },
    tokens     = { "core:command_tokens", numbered = 3, decoys = 1 },
    activation = { "core:activation", steps = { "move", "attack" } },
    movement   = { "core:movement", headroom = "figure_height" },
    engagement = { "core:engagement" },
    combat     = { "core:combat", height_bonus = 1 },
    los        = { "core:los", policy = "geometric" },
    relics     = { "core:relics" },
    units      = { "core:units" },
    deployment = { "core:deployment" },
    army       = { "core:army" },
    victory    = { "core:victory" },
  },
  undo = "until_random_or_reveal",
}

ruleset {
  id = "intro",
  extends = "advanced",
  slots = { tokens = false, relics = false, combat = { "core:combat", height_bonus = 0 } },
  abilities = false,
}

-- In a mod:
ruleset {
  id = "ambush",
  extends = "strata_std:advanced",
  slots = { combat = { "defender_first", min_wounds = 1 } },
  addons = { { "morale", break_at = 2 } },
}
```

`false` empties a slot. A module with a hard dependency on an empty slot is a load error, so `core:activation` treats `tokens` as optional and lets players pick units freely when it is empty.

### 6.4 Phases

Game flow is a tree of phases: setup, then rounds, each containing initiative, token placement, turns and round end. Entering and leaving a phase emits events, which is what "at the start of your turn" hooks. Modules insert phases at named anchors (`after = "turns.round_end"`).

### 6.5 Actions

An activation gives each figure a budget of slots, by default one `move` and one `attack`. Every action declares the slots it consumes, its targeting, and its resolver. The ordinary move and attack are themselves actions from the movement and combat modules. A special attack is just another action that consumes `attack`, and "may move again after attacking" grants one extra `move`.

### 6.6 Queries, modifiers and descriptors

Stats are never stored as current values. They are computed by queries, which is what makes abilities composable. A query starts from the base value, applies modifiers by layer (`set`, `add`, `mul`, `final`), then clamps to the stat's bounds. Boolean queries such as `can_enter`, `is_engaged`, `can_target` and `provokes_reprisal` collect votes, and `deny` beats `allow` beats the default unless a modifier is marked `override`.

This is where the hot-path design lives. A modifier's condition, selector and value are usually *descriptors*: tagged tables such as `cond.adjacent { to = "self" }` or `sel.allies { trait = "soldier" }`, which the Rust evaluator handles natively and the AI can read. Any of those fields also accepts a plain Lua function, which is the slow path for cases no descriptor covers. Because archetypes (section 8) are built from descriptors, most queries never enter Lua.

Results are memoized within a resolution step and invalidated whenever an event applies.

### 6.7 Triggers, replacements and windows

- **Triggers** react after an event applies, with an optional filter and a limit of once per turn, round or game.
- **Replacements** run before an event applies and may modify, replace or cancel it. Each applies at most once per event, which prevents loops. When several apply, the affected figure's controller orders them.
- **Windows** are named pauses inside a resolution where eligible reactions are offered as decisions, such as `combat.before_defense_roll` and `combat.after_wounds`.

### 6.8 Dice

Dice are definitions: `die { id = "combat", faces = { "hit", "hit", "hit", "block", "block", "blank" } }`. Pool sizes come from queries, rolls emit `DiceRolled`, and a reroll is a decision followed by a replacement roll.

## 7. Board geometry

The board is a sparse set of hex columns, each a stack of terrain segments, and every geometric test runs in exact integer math inside the kernel crate. Line of sight and movement are therefore identical on every machine, and Bevy's physics is never consulted.

### 7.1 Coordinates

A column is addressed by axial coordinates (q, r), a pair of integers naming one hex in the plane. The implied third cube coordinate is s = −q − r. With Δq, Δr and Δs the coordinate differences between columns a and b, hex distance is:

```latex
d(a, b) = \frac{|\Delta q| + |\Delta r| + |\Delta s|}{2}
```

Vertical position is an integer level z ≥ 0, where one level is one tile thickness. A *space* is a triple (q, r, z) naming a surface a figure can stand on. Flat-top versus pointy-top orientation is purely a rendering choice.

### 7.2 Columns, segments and surfaces

Each column holds an ordered list of solid segments. A segment covers the half-open level interval [z₀, z₁), from bottom level z₀ up to top level z₁, and has a terrain type. A surface exists at z₁ when the gap to the next segment's bottom is at least one level, or when there is no segment above.

Several segments per column give overhangs for free. A bridge is a column with ground from level 0 to 1 and a deck from level 4 to 5, which produces two spaces at the same (q, r). Whether a figure fits under an overhang is a movement-module setting (`headroom = "ignore"` or `"figure_height"`).

Non-terrain objects such as trees, walls and ruins are features: separate objects with their own segments and flags (section 10.3). At match start the map compiles into a dense grid over its bounding box, shared behind an `Arc`.

### 7.3 Footprints

A footprint is a set of axial offsets from an anchor cell, plus a rotation o ∈ {0, …, 5} in 60-degree steps. A one-space base is {(0, 0)} and a two-space base is {(0, 0), (1, 0)}; mods can define any connected shape. Every cell must sit on a surface, and the largest allowed level spread across the cells is a footprint setting. Multi-space figures move in `snake` mode, where the rear follows the leader's path, or `rigid` mode, where the shape translates.

### 7.4 Movement

The movement graph is built per query, never stored. Nodes are (space, rotation) pairs, edges join surfaces in adjacent columns, and edge cost and legality come from the `move_cost`, `can_enter` and `can_climb` queries.

`reachable(figure)` runs Dijkstra over integer costs. It returns each reachable space with its cost, its path, and flags such as `stops_here`, `provokes_reprisal` and `falls`. To keep this native, the movement module caches edge costs per (figure, terrain type, level change) at the start of each query, so a Lua-defined cost is called once per combination rather than once per edge. The UI preview and the AI both read the same result.

### 7.5 Line of sight

The trick is to work in axial coordinates scaled by 3. A hex corner is the centroid of three mutually adjacent hex centers, so its axial coordinates have denominator 3. After scaling, every center and corner is an integer point. The axial-to-world map is affine, so straight lines stay straight, and every intersection test reduces to integer cross products compared as fractions in `i128`. (Implementation note: the 80% inset hexes of section 7.6 need a scale of 15 so inset corners are integers too.)

Each figure has a LOS profile (section 10.4): a target point, and a hit zone with sample points. Heights use sub-level units, 12 per level. For each hit-zone sample on the defender, the kernel casts a segment from the attacker's target point:

1. Walk the segment's 2D projection through the hex grid with an exact supercover traversal.
2. For each column crossed between the endpoints, compute where the segment enters and leaves it.
3. Height varies linearly along the segment, so each stretch covers one height interval. The stretch is blocked if that interval overlaps the interior of a solid segment, a blocking feature, or a blocking figure in that column.
4. Touching a boundary does not block. That fixed tie rule makes edge-grazing cases resolve identically every time.

LOS exists when the policy's threshold of sample segments is clear (`any` by default). The kernel returns the clear segments as witnesses for the client to draw. A simpler `columns` policy (center to center) ships too, and a Lua module can provide its own by taking the `los` slot.

### 7.6 Line of sight with sprites

Figures are drawn as sprites (section 13.8), but line of sight never looks at a sprite. Rules cannot depend on the camera: rotating the view must not change who can see whom, and the AI and the CLI have no camera at all. So each figure's LOS profile is a camera-independent volume, and the sprite pipeline guarantees the picture matches it.

- **The volume.** A figure's hit zone is a vertical prism over its footprint cells, inset to 80% of the hex so neighbors never touch. It runs from the bottom to the top of `hit_zone`, in sub-level units. The target point sits at `target`, the figure's eye height, above the anchor cell's center.
- **Sample points.** Samples sit at three heights (just above the bottom, the middle, just below the top), each at the center and at the six inset corners: 21 points for a one-space figure. A head peeking over a wall is visible in the rules exactly as it is on screen.
- **Partial cover.** The combat module can read how many samples were blocked. Its `cover` setting, off by default, grants +1 defense when at least half are blocked.
- **The picture must match.** Every sprite declares its foot anchor, and each pack declares its pixels per level. `strata lint` measures each sprite's opaque height in every direction and fails if it differs from the figure's hit-zone top by more than one sub-level unit, or if it spills wider than its footprint. A sprite that towers over a wall on screen is therefore above that wall in the rules.
- **Teaching the shape.** The attack preview outlines the defender's volume as a faint prism around its sprite and draws the witness rays to it (section 13.6).

Casting against each sprite's flat billboard was considered and rejected. It would match the screen pixel for pixel, but it makes line of sight depend on the camera angle, which breaks replays, the AI, and fairness between two players looking from different sides.

## 8. Ability archetypes

Most abilities in this genre are variations on eight shapes. The standard pack provides each shape as a parameterized Lua constructor built from descriptors. A typical ability is therefore one call that writes its own card text, evaluates natively in Rust, and can be read by the AI. Unique abilities are a few lines of inline Lua in the unit's file.

### 8.1 The eight families

| Archetype | Covers | Typical parameters | Built on |
| --- | --- | --- | --- |
| `stat_bonus` | Conditional bonuses to the figure itself: high ground, against a trait, while engaged | `stat`, `amount`, `when`, `against` | Modifier |
| `aura` | Bonuses or penalties to other figures nearby | `stat`, `amount`, `to`, `range` | Modifier |
| `special_attack` | Attacks that replace the normal one: other range or dice, several targets, areas, lines | `range`, `dice`, `targets`, `area`, `self_wounds` | Action on the `attack` slot |
| `movement_mode` | Flying, passing through figures, terrain immunity, no reprisals, swapping places | `ignores`, `immune_to`, `no_reprisal` | Movement and engagement modifiers |
| `reaction` | Responses inside a window: negate wounds on a roll, counter-wound the attacker, dodge | `window`, `when`, `roll`, `effect` | Window reaction or replacement |
| `roll_trigger` | "After X, roll the d20; on N or more, Y" | `after`, `roll`, `effect`, `limit` | Trigger plus decision |
| `activation_trick` | Activating another unit alongside this one, extra moves, acting out of order | `with`, `timing`, `grants` | Phase hook or slot grant |
| `board_effect` | Healing, placing or summoning figures, reviving, moving others, taking control, status markers | `effect`, `targets`, `timing` | Action or trigger |

Each archetype is ordinary Lua library code in `strata_std` that returns an `ability` definition. A modder adds a ninth family by writing one more function; the engine never changes.

### 8.2 Descriptor vocabulary

Archetype parameters are filled with descriptors from four namespaces: `sel` (which figures or spaces), `cond` (when), `val` (how much) and `eff` (what happens). Each descriptor provides three things: a native Rust evaluator, a card-text phrase, and a shape the AI can read.

| Namespace | Examples |
| --- | --- |
| `sel` | `self`, `allies`, `enemies`, `my_units`, with filters `adjacent_to`, `within`, `in_los`, `trait`, `engaged` |
| `cond` | `higher_than`, `on_terrain`, `engaged`, `attack_kind`, `phase`, `count`, `all`, `any`, `none` |
| `val` | Literal integers, `stat`, `count`, `level_difference`, `distance`, `sum`, `min`, `max` |
| `eff` | `wound`, `heal`, `place`, `remove`, `move`, `grant_slot`, `add_marker`, `take_control`, `cancel_wounds`, `excess_blocks_wound_attacker` |

Any parameter also accepts a plain Lua function. That is the slow path: it works everywhere, costs a Lua call per evaluation, and is opaque to the AI.

### 8.3 Archetypes in use

```lua
local kw = require("strata_std.keywords")   -- shared, named abilities

unit {
  id = "ironclad_line",
  -- stats, figures, etc. as in section 10.4
  abilities = {
    aura { name = "Shieldwall", stat = "defense", amount = 1,
           to = sel.allies { adjacent_to = "self", trait = { class = "soldier" } } },
    roll_trigger { name = "Follow-through", after = "attack", roll = { "d20", at_least = 16 },
                   effect = eff.grant_slot("attack"), limit = "once_per_turn" },
    kw.disciplined,
  },
}
```

The other families, one line each:

```lua
stat_bonus       { name = "High Ground", stat = "attack", amount = 1, when = cond.higher_than("target") }
special_attack   { name = "Volley", range = 6, dice = 3, targets = { count = 3, distinct = true } }
special_attack   { name = "Blast", range = 5, dice = 4, area = { radius = 1, friendly_fire = true } }
movement_mode    { name = "Flight", ignores = { "elevation", "figures", "stop_terrain" } }
reaction         { name = "Evasion", window = "before_wounds", when = cond.attack_kind("ranged"),
                   roll = { "d20", at_least = 17 }, effect = eff.cancel_wounds() }
reaction         { name = "Riposte", window = "after_defense_roll", when = cond.attack_kind("melee"),
                   effect = eff.excess_blocks_wound_attacker() }
activation_trick { name = "Rally", timing = "before_or_after",
                   with = sel.my_units { trait = { class = "soldier" } } }
board_effect     { name = "Mend", action = "attack", targets = sel.allies { within = 1 }, effect = eff.heal(2) }
```

Shared keywords are just archetype calls bound to names in a Lua module. `kw.flight` is a value; `kw.flight { no_reprisal = true }` is the same keyword with an override.

### 8.4 Card text

Each archetype has a text template, and each descriptor contributes a phrase, so Shieldwall renders as "Adjacent friendly soldier figures add 1 to their defense." An explicit `text = "..."` overrides the generated wording. `strata lint` warns when an override omits a number that appears in the parameters, which catches text drifting away from behavior.

### 8.5 Unique abilities

An ability no archetype fits is written directly with `ability {}`. Its hook groups mirror the seams in section 6: `modifiers`, `actions`, `before` (replacements, keyed by event), `after` (triggers, keyed by event), `reactions` (keyed by window), and `vars` (per-instance state with defaults).

```lua
ability {
  name = "Grave Pact",
  text = "When an ally within 3 is destroyed, you may place 1 wound on an enemy within 3 of it.",
  after = {
    figure_destroyed = function(ctx, self, e)
      if not ctx.is_ally(self, e.figure) or ctx.distance(self, e.space) > 3 then return end
      local enemies = ctx.select(sel.enemies { within = 3, of = e.space })
      if #enemies == 0 then return end
      local target = ctx.choose { prompt = "Place a wound", figures = enemies, optional = true }
      if target then ctx.units.wound(target, 1, { source = self }) end
    end,
  },
}
```

Mixing is fine. An archetype with one Lua function in a single parameter keeps everything else native: `stat_bonus { stat = "attack", amount = 2, when = function(ctx, self) return ctx.vars(self).charged end }`.

### 8.6 Measuring coverage

`strata stats abilities <pack>` reports how many abilities are pure archetype, archetype plus a Lua function, or fully custom. In dev builds, a profiler counts slow-path query calls per ability during self-play. Together they show when a new archetype or descriptor is worth adding, and they feed the M6 coverage report (section 17).

## 9. Lua runtime

Lua runs in disposable, sandboxed states, one per simulation thread, and every piece of game state lives in Rust. A script can read the game, ask questions, and emit effects through `ctx`, but it cannot hold state of its own. That one rule is what makes replays, AI cloning and multi-threading safe.

### 9.1 Loader and runtime states

The **loader state** runs the data stages once (section 4.3) and is dropped after the freeze.

**Runtime states** run the hooks. There is one on the session thread, one per AI worker, and one per test runner. Each loads the same pack scripts in runtime mode, harvests the functions inside the definitions, and binds them to their `ScriptRef`s by path, such as `unit:ironclad_line/abilities/2/after/figure_destroyed`. A hash of the harvested paths must match the registry, or the state is rejected. Runtime states are cheap, so the AI simply creates more.

### 9.2 API surface

| Name | Purpose |
| --- | --- |
| `ctx.state` | Read-only views of figures, units, board, round and phase; writes raise an error |
| `ctx.select(sel)` | Evaluates a selector; always returns an array sorted by ID |
| `ctx.query.*` | Runs a query with every modifier applied (`ctx.query.stat(fig, "defense")`) |
| `ctx.choose { }` | Asks a decision; answered from the tape or halts (section 5.2) |
| `ctx.dice.roll(die, n)` | Returns a roll with `:count(symbol)`, `:sum()` and `:faces()` |
| `ctx.units`, `ctx.movement`, ... | Effects exported by modules; each call emits events |
| `ctx.vars(owner)` | Per-instance variables; every write becomes a `VarSet` event |
| `ctx.emit(kind, data)` | Emits a custom event |
| `ctx.log(...)` | Rules-trace output in dev builds; no effect on state |

### 9.3 Sandbox

- **Libraries kept:** base, `string`, `table`, `math`, `utf8`.
- **Removed:** `io`, `os`, `debug`, `package`, `coroutine`, `dofile`, `loadfile`, `load`, `collectgarbage`, `math.random` and `math.randomseed`.
- **`require`** resolves only inside the pack and its declared dependencies. Module tables are frozen after loading.
- **Environments** are per pack and frozen at runtime, so writing a global raises an error.
- **Budgets:** `mlua`'s instruction-count hook aborts any call over its budget, and `set_memory_limit` caps each state's memory. Counting instructions rather than time keeps aborts deterministic. An aborted call rejects the command and flags the ability in the dev overlay.

### 9.4 Determinism rules

- **Integers only.** Lua 5.4 has an integer subtype, and the bridge rejects any float crossing into Rust. `/` always produces a float, so scripts use `//`, and the linter flags `/`.
- **Ordered iteration.** Lua 5.4 seeds string hashing per state, so raw table order can differ between threads. The runtime replaces `pairs` with a version that walks keys in sorted order, and removes `next`.
- **No hidden state.** Frozen environments stop global writes, but a closure could still mutate an upvalue (`local n = 0` captured by a hook), and that value would then differ between the session and AI states. The linter rejects assignments to upvalues inside hooks; per-instance state belongs in `ctx.vars`. The determinism harness (section 16.5) catches anything the linter misses.
- **No clocks or randomness** except `ctx.dice` and the command RNG.

### 9.5 Editor support

`strata stubs` generates LuaLS `---@meta` files for every constructor, archetype, descriptor and `ctx` namespace, with parameter types. VS Code, Neovim and Zed then show autocomplete and type warnings. The stubs come from the same schemas the loader validates against, so they cannot drift.

## 10. Data model: definitions

Every definition is a Lua constructor call during the data stage, which freezes into a typed Rust struct. The constructors are the authoring surface, and the Rust structs are what the rules read.

### 10.1 Conventions

- Every constructor takes `id` and `name`, plus optional `tags` and `meta`. The engine ignores `meta`; tools may use it.
- Stats are a table keyed by stat IDs, never fixed fields, so a mod adds a stat by defining one.
- All numbers are integers. LOS heights use sub-level units, 12 per level.
- Each constructor's schema lives in Rust. It drives validation, error messages and the LuaLS stubs.

| Constructor | Purpose |
| --- | --- |
| `stat`, `die`, `marker`, `traits`, `faction` | Vocabulary: stats with bounds, dice faces, wounds and statuses, allowed trait keys, allegiances |
| `terrain`, `tile`, `feature` | Surface behavior, physical pieces for the editor, non-terrain board objects |
| `footprint`, `unit` | Base shapes and move modes; selectable units with figures and abilities |
| `ability` plus the archetypes | Behavior (section 8) |
| `relic` | Claimable board objects |
| `rule_module`, `ruleset` | Rules composition (section 6) |
| `map`, `inventory`, `scenario`, `format`, `army` | Matches (section 11) |
| `test` | Pack tests (section 16) |

### 10.2 Vocabulary

```lua
stat    { id = "defense", name = "Defense", min = 0, max = 99, abbrev = "DEF", order = 5 }
die     { id = "combat", name = "Combat die", faces = { "hit", "hit", "hit", "block", "block", "blank" } }
die     { id = "d20", name = "d20", range = { 1, 20 } }
marker  { id = "wound", name = "Wound", stacking = "count", visibility = "public" }
traits  { id = "std_traits", keys = { species = "open", class = "open", temperament = "open",
                                     size = { "small", "medium", "large", "huge" } } }
faction { id = "brass_compact", name = "The Brass Compact", color = "#B5893A", emblem = "icon.brass_compact" }
```

### 10.3 Terrain, tiles and features

Terrain behavior is expressed as abilities attached to the terrain, so hazards reuse the ability system rather than growing a second one. Inside a terrain ability, `sel.occupant` names the figure standing on it.

```lua
terrain {
  id = "magma", name = "Magma", standable = true, tags = { "hazard" },
  abilities = { hazard_roll { timing = "turn_end", die = "d20", fail_below = 8, wounds = 1 } },
}

tile { id = "grass_7", terrain = "grass", thickness = 1, shape = hex.shapes.seven }

feature {
  id = "ruin_arch", name = "Ruined arch", blocks_los = true,
  cells = {
    { at = { 0, 0 }, segments = { { 0, 3 } } },
    { at = { 1, 0 }, segments = { { 2, 3, top = true } } },   -- deck you can stand on, gap beneath
    { at = { 2, 0 }, segments = { { 0, 3 } } },
  },
}
```

Tile shapes are lists of axial offsets; `hex.shapes` holds the common ones. Feature segments are in levels, measured from the surface the feature is placed on.

### 10.4 Footprints and units

```lua
footprint { id = "two", cells = { { 0, 0 }, { 1, 0 } }, move = "snake", max_spread = 1 }

unit {
  id = "ironclad_line",
  name = "Ironclad Line",
  kind = "squad",
  rarity = "common",
  faction = "brass_compact",
  traits = { species = "human", class = "soldier", temperament = "disciplined", size = "medium" },
  stats = { life = 1, move = 5, range = 1, attack = 2, defense = 4, points = 60 },
  figures = {
    { id = "trooper", count = 4, footprint = "one", height = 5,
      los = { target = 54, hit_zone = { 0, 60 }, samples = { "center", "corners" } },
      visual = "sprite.ironclad_trooper" },
  },
  abilities = { --[[ section 8.3 ]] },
  card = { art = "art.ironclad_line", flavor = "Bolted together, they hold together." },
}
```

```rust
pub struct UnitDef {
    pub id: DefId,
    pub name: Text,
    pub kind: Sym,                      // vocabulary owned by the units module
    pub rarity: Sym,
    pub faction: Option<DefId>,
    pub traits: BTreeMap<Sym, Sym>,
    pub stats: BTreeMap<DefId, i32>,
    pub figures: Vec<FigureDef>,
    pub abilities: Vec<AbilityDef>,
    pub card: CardInfo,
    pub source: SourceLoc,              // file and line, for every later error
}

pub struct FigureDef {
    pub id: Sym,
    pub count: u8,
    pub footprint: DefId,
    pub height: u8,                     // levels
    pub los: LosProfile,                // sub-level units
    pub visual: Option<Sym>,
}
```

A hero with distinct figures lists several `figures` entries. `kind` and `rarity` are vocabulary owned by the units and army modules, not the engine.

`visual` names a sprite set (section 13.8). The `los` profile is the rules' truth, and the sprite is linted against it (section 7.6).

### 10.5 Abilities, frozen

```rust
pub struct AbilityDef {
    pub name: Text,
    pub text: Text,                          // generated or overridden (section 8.4)
    pub scope: Scope,                        // Figure, Unit, Army, Holder or Occupant
    pub archetype: Option<ArchetypeInfo>,    // family and parameters, for the AI and coverage stats
    pub modifiers: Vec<ModifierDef>,
    pub actions: Vec<ActionDef>,
    pub replacements: Vec<ReplacementDef>,
    pub triggers: Vec<TriggerDef>,
    pub reactions: Vec<ReactionDef>,
    pub vars: BTreeMap<Sym, Value>,
}

/// Every condition, selector, value and effect is one of these.
pub enum Logic {
    Native(Descriptor),    // evaluated in Rust, readable by the AI
    Script(ScriptRef),     // a Lua function: the slow path
}
```

`Logic` puts the fast and slow paths into the type system. Coverage stats (section 8.6) simply count the two variants.

### 10.6 Relics

```lua
relic {
  id = "fury", name = "Relic of Fury",
  mode = "while_occupied",            -- or "on_claim" with an `effect`
  claim = "ends_move_on",
  face_down_allowed = true,
  grants = { to = "occupant_army", abilities = { stat_bonus { stat = "attack", amount = 1 } } },
  visual = "relic.fury",
}
```

An instant relic uses `mode = "on_claim"` with an `effect` instead of `grants`. A scenario objective is a relic with no grants that victory conditions reference by ID.

## 11. Data model: maps, scenarios, formats, army lists

Four definitions describe a match, each wrapping the one before. A map is pure geometry; a scenario adds deployment, objectives and scripted events; a format is the competitive envelope; an army list is one player's picks, validated against a format.

### 11.1 Map

The map editor writes maps as Lua table literals: plain data, no functions. A map's single source of truth is its list of placed pieces, and columns (section 7.2) are compiled from it at load. Freeform painting is just placing one-hex tiles with no inventory limit, so there is never a second representation to keep in sync.

```lua
-- maps/twin_fords.lua: written by the editor. Hand edits load fine, but the editor rewrites the file on save.
return map {
  id = "twin_fords", name = "Twin Fords", authors = { "liam" }, orientation = "pointy",
  pieces = {
    { "grass_24", at = { 0, 0 },  rot = 0, level = 0 },
    { "rock_3",   at = { 4, -1 }, rot = 2, level = 1 },
    { "water_1",  at = { 6, 2 },  rot = 0, level = 1 },
  },
  features = { { "ruin_arch", at = { 5, 1 }, rot = 0 } },
  zones = {
    start_a    = { { -6, 2 }, { -6, 3 }, { -5, 2 } },
    start_b    = { { 7, -3 }, { 7, -2 }, { 6, -2 } },
    ford_north = { { 2, -1 }, { 3, -1 } },
  },
  relic_spots = { center = { 1, 0 } },
  constraints = { inventory = "starter_box", require_support = true },
}
```

`level` is a piece's bottom level. Overlapping pieces are a load error. With `require_support`, every cell must rest on level 0 or on solid terrain, which mirrors physical building. Zones are named sets of columns that scenarios refer to by name.

An inventory caps the pieces a map may use, for builds a player can reproduce on a table:

```lua
inventory { id = "starter_box", tiles = { grass_1 = 10, grass_3 = 8, grass_24 = 2, rock_3 = 6, water_1 = 6 },
            features = { tree_small = 4 } }
```

### 11.2 Scenario

Scenarios are hand-written, which is where "Lua for everything" pays off: one-off events sit inline next to the setup they affect.

```lua
scenario {
  id = "hold_the_fords", name = "Hold the Fords", map = "twin_fords",
  players = { min = 2, max = 2 },
  slots = {
    { id = "a", start = "start_a", deploy_order = 1 },
    { id = "b", start = "start_b", deploy_order = 2 },
  },
  army = { army_rule.max_points(400) },
  relics = { { "fury", spot = "center" } },
  victory = {
    win.eliminate_all(),
    win.hold_zone("ford_north", { rounds = 2 }),
    win.round_limit(12, { tiebreak = "points_remaining" }),
  },
  addons = { { "reinforcements", round = 4, zone = "start_a" } },
  after = {
    round_started = function(ctx, e)
      if e.round == 3 then ctx.board.remove_feature("ruin_arch") end
    end,
  },
}
```

`win.*` constructors are victory conditions registered by modules. Each is evaluated after every command and at round end, and may supply a `progress(state, player)` estimate for the AI. The first listed condition wins ties.

### 11.3 Format

A format is everything a tournament organizer decides. A scenario may tighten a format's army rules but never loosen them, because both sets always apply.

```lua
format {
  id = "duel_500", name = "Duel, 500 points",
  ruleset = "advanced",
  army = { army_rule.max_points(500), army_rule.unique_once(), army_rule.banned {} },
  selection = "build",                 -- or "draft_snake"
  scenarios = { pool = { "hold_the_fords" }, pick = "alternate_veto" },
  round_limit = 12,
  clock_minutes = 45,
  tiebreakers = { "points_destroyed", "initiative_roll" },
}
```

`army_rule.*` constructors cover point caps, uniqueness, copy limits, bans, faction requirements and total footprint. A mod adds a new constraint with a Lua function.

### 11.4 Army list

The army builder writes army lists, also as Lua table literals.

```lua
return army {
  name = "Brass and Thunder",
  format = "strata_std:duel_500",
  packs = { strata_std = "0.1.0", liam_originals = "0.3.0" },
  units = {
    { "liam_originals:ironclad_line", copies = 2 },
    { "liam_originals:marshal_varro", copies = 1 },
  },
}
```

Point totals are always computed, never stored, so a balance patch reprices saved lists automatically. Validation returns every violation with the army rule that produced it, and the builder shows them inline.

## 12. Data model: runtime game state

Runtime state holds only what changes during a match; everything static is an `Arc` into the frozen registry or the compiled board. State is a plain `Clone` struct, small enough that the AI copies it thousands of times per turn, and every ID is a typed newtype so a figure ID can never be passed as a unit ID.

### 12.1 Top level

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)] pub struct FigureId(u32);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)] pub struct UnitId(u32);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)] pub struct PlayerId(u8);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)] pub struct Space { pub q: i32, pub r: i32, pub z: i32 }

#[derive(Clone, StableHash)]
pub struct GameState {
    pub setup: Arc<MatchSetup>,          // immutable inputs, see 12.2
    pub board: Arc<CompiledBoard>,       // static geometry
    pub board_changes: BoardChanges,     // removed features, occupancy index
    pub command_index: u32,
    pub players: Vec<PlayerState>,
    pub units: Vec<UnitState>,           // indexed by UnitId
    pub figures: Vec<FigureState>,       // indexed by FigureId
    pub relics: Vec<RelicState>,
    pub abilities: Vec<AbilityInstance>, // attached instances: def, owner, resolved params
    pub flow: FlowState,
    pub vars: BTreeMap<VarKey, Value>,   // module and ability variables, namespaced
    pub pending: Option<PendingDecision>,
}
```

### 12.2 Match setup

The inputs fixed before the first command. Together with the command log, they fully determine the game.

```rust
pub struct MatchSetup {
    pub packs: Vec<PackIdentity>,        // id, version, content hash
    pub ruleset: DefId,
    pub format: Option<DefId>,
    pub scenario: DefId,
    pub seed: SeedSource,                // a plain seed locally; commit-reveal if online play arrives
    pub players: Vec<PlayerSetup>,       // id, scenario slot, team, army list
}
```

### 12.3 Figures, units and players

| `FigureState` field | Type | Notes |
| --- | --- | --- |
| `unit` | `UnitId` | Owning unit |
| `def` | `FigureDefRef` | Unit def plus figure entry |
| `location` | `Option<Space>` | Anchor space; `None` when off the board |
| `rotation` | `u8` | 0 to 5 |
| `status` | enum | `Reserve`, `OnBoard`, `Destroyed`, `Removed` |
| `controller` | `PlayerId` | Differs from the unit's owner while under another player's control |
| `markers` | `SmallVec<[Marker; 4]>` | Wounds and statuses: def, count, source, expiry |

A `UnitState` holds its def, owner, copy index, figure IDs, and the tokens on its card. A `PlayerState` holds team, tokens in hand, an eliminated flag and score counters. Whether a player is human, AI or remote lives outside the core entirely.

### 12.4 Flow and pending decisions

`FlowState` is a stack of active phases (each with its own small data), plus round, turn-in-round, active player and the current activation. An activation records the unit, the current step, and each figure's remaining slots.

`PendingDecision` holds the decision, its chooser, the command being resolved, and the answers so far. It is part of state, so a save taken mid-decision restores to the same question.

### 12.5 Hidden information

Token assignments carry a visibility tag. `PlayerView` projects state per player, and the event stream is filtered the same way (section 15.3).

### 12.6 Saves and replays

A save and a replay are the same thing: engine version, pack identities, setup, and the command log, with optional snapshots for fast seeking. They are machine files rather than content, so they use serde with `postcard` for compactness. `strata replay dump` prints any replay as readable text.

## 13. Bevy client

Bevy renders and takes input; the rules run elsewhere. The client talks to a simulation thread through two channels, and ECS entities exist only to draw what the event stream says happened.

### 13.1 The simulation thread

The session runs on its own OS thread: state, the runtime Lua state, controllers and the command loop. The client sends commands and decision answers in, and receives event batches and player views out, over `crossbeam-channel`. Bevy systems never touch Lua, which matters because an `mlua` state is not `Send` by default. They also never block on a long resolution or an AI turn. AI workers run on further threads with their own Lua states.

### 13.2 Why the rules are not ECS systems

Bevy's ECS is the right tool for rendering and input but the wrong one for these rules, for two reasons. First, resolution is a sequential call stack that pauses for decisions mid-step, which scheduled systems express poorly. Second, the AI must clone the entire game thousands of times per turn, and a Bevy `World` cannot be cloned cheaply. Figures therefore appear in ECS only as render entities carrying a `FigureView { id: FigureId }` component.

### 13.3 Plugins

| Plugin | Responsibility |
| --- | --- |
| `SimLinkPlugin` | Starts the simulation thread; drains its channel into a `PendingEvents` resource each frame |
| `BoardPlugin` | Builds terrain meshes from the compiled board: one merged mesh per terrain material, side walls only on exposed faces |
| `FigurePlugin` | Spawns figure sprites as upright, camera-facing billboards with directional frames, or generated fallbacks |
| `PlaybackPlugin` | Plays event batches in sequence (moves, dice, markers); locks input meanwhile; offers skip |
| `InputPlugin` | Picks hexes with kernel ray math against the columns and figures by sprite bounds, then turns clicks into commands checked against `legal_commands` |
| `CameraPlugin` | Orthographic isometric camera: six views in 60-degree steps, integer zoom levels, pan |
| `UiPlugin` | `bevy_egui` panels: unit cards, army builder, decision prompts, game log |
| `PreviewPlugin` | Movement reach, LOS rays and combat odds |
| `EditorPlugin` | The map editor (section 14) |
| `AssetsPlugin` | Resolves visual keys through each pack's `assets.lua`, with fallbacks |
| `DevPlugin` | Rules inspector, error overlay, hot reload |
| `OcclusionPlugin` | Draws silhouettes of figures hidden behind terrain, and fades the columns between the camera and the selected figure |

`bevy_egui` handles all UI in the prototype, because it is the fastest way to build dense tool-style panels. In-game UI polish can move to Bevy's native UI later without touching anything outside `strata_client`.

### 13.4 Asset fallbacks

Each pack's `assets.lua` maps visual keys (`sprite.ironclad_trooper`, `art.ironclad_line`, `terrain.magma`) to files and sprite metadata. A missing key gets a generated placeholder:

- Figures become generated silhouette sprites: a faction-colored capsule drawn at the figure's hit-zone height in all six directions, standing on a base that matches the footprint. They pass the sprite lint by construction.
- Terrain becomes tinted hex prisms, with the terrain name on hover.
- Card art becomes the faction emblem on a solid color.

A data-only pack is therefore playable the moment it validates.

### 13.5 Generic decision UI

Each decision type maps to one UI pattern. A figure or space choice highlights the legal options on the board, an option choice opens a dialog, a number choice opens a stepper, and an ordering choice opens a drag list. A modder's `ctx.choose` gets correct UI with no UI code.

### 13.6 Previews

- **Movement** shades `reachable` results, labels path costs, and flags spaces that provoke reprisals or stop movement.
- **Attack** outlines the defender's LOS volume around its sprite, draws the witness rays to it, and shows both dice pools. It also shows the exact distribution of wounds, computed by convolving the two dice definitions with no sampling.

### 13.7 Containing Bevy upgrades

Bevy ships breaking releases every few months. The workspace pins one Bevy version, and every Bevy-dependent crate (including `bevy_egui`) is used only inside `strata_client`, so an upgrade touches one crate.

### 13.8 Isometric rendering and sprites

The client is 2.5D: real 3D hex terrain seen through an orthographic camera, with every character drawn as a sprite. Terrain gets stacking, overhangs and rotation for free from being 3D, and characters get the look and cost of pixel art.

- **Camera.** An orthographic camera at a fixed elevation chosen to match the sprite art (30 degrees is a good start), with six yaw steps of 60 degrees. Zoom moves in integer steps so pixels stay crisp, and textures use nearest-neighbor filtering.
- **Terrain.** Hex-prism meshes rendered orthographically, with flat shading and textures matched to the sprites' palette.
- **Figures.** Upright billboards in the 3D scene: quads that turn with the camera's yaw but never tilt. They are alpha-cutout, so the depth buffer sorts them against terrain correctly. Each is anchored at its foot point on the figure's surface.
- **Directions.** Every sprite set has six directions, one per hex facing. The frame shown is the figure's facing minus the camera's yaw, modulo 6.
- **Animations.** `idle` is required; `move`, `attack`, `hit` and `death` fall back to `idle` when missing.
- **Scale.** Each pack declares its pixels per level and the hex width in pixels, so sprites line up with terrain on every pack.

```lua
-- assets.lua
pixels_per_level = 12
hex_width_px = 64

sprite {
  key = "sprite.ironclad_trooper",
  sheet = "sprites/ironclad_trooper.png",
  frame = { w = 48, h = 72 },
  anchor = { x = 24, y = 68 },          -- foot point, in pixels
  directions = 6,
  animations = {
    idle   = { frames = 4, fps = 6 },
    move   = { frames = 6, fps = 10 },
    attack = { frames = 5, fps = 12 },
  },
}
```

Sprite metadata is presentation data. It never enters the rules registry or the content hash, so changing art cannot break a replay.

## 14. Map editor

Maps are the one content type that needs a UI, because nobody should type hex coordinates by hand. The editor's logic lives in a Bevy-free crate, `strata_editor`, and its UI is a client plugin. Every edit is therefore unit-testable, and the CLI can validate maps headlessly.

### 14.1 Editor core (`strata_editor`)

- **Document.** A `MapDocument` wraps the map being edited, plus compiled columns that refresh after each change.
- **Edit commands.** Place, remove, move and rotate a piece; place a feature; paint a zone; set a relic spot; set metadata. Each implements `apply` and `undo`, which gives unlimited undo and redo.
- **Live validation.** Overlapping pieces, unsupported cells, inventory overruns, zones on unstandable columns, start zones too small for the format's largest army, and relic spots unreachable from any start zone. Each finding carries a severity and the cells it concerns.
- **Analysis.** Reachability between start zones for a reference one-space figure, and a fairness report comparing path lengths from each start zone to each objective.
- **Writer.** Saves a Lua table literal with stable key order and one piece per line, so maps diff cleanly in git.

### 14.2 Editor UI (`EditorPlugin`)

- A **palette** built from the loaded packs' tiles and features, with remaining counts when an inventory is set.
- A **placement ghost** with rotation, snapping to the top surface under the cursor or locked to an explicit level.
- **Brushes** for freeform mode: single hex, line, and region fill.
- **Level slicing**, which hides everything above a chosen level so arch interiors and bridge undersides stay editable.
- A **symmetry tool** that mirrors or rotates each placement, for fair maps of two to six players.
- A **zone painter** with named, colored layers, plus a relic-spot tool.
- A **validation panel**; clicking a finding frames the camera on its cells.
- A **playtest button** that starts a hotseat match on the current map immediately, since the core needs no scene setup.

### 14.3 Scenarios from a map

Scenarios are hand-written Lua, but the editor can generate a skeleton for the current map. The skeleton includes one slot per start zone, relics on every spot, and default victory conditions, saved to `scenarios/` for hand editing.

## 15. AI, replays and hidden information

All three consume the same deterministic core, which is why they are cheap to add. The AI simulates rather than special-casing abilities, so modded abilities work for it automatically. Archetype-built abilities are also readable, which makes its estimates sharper.

### 15.1 Controllers

Each seat has a controller that produces commands and answers decisions from that player's view. The session never knows which kind it is talking to.

```rust
pub trait Controller: Send {
    fn choose_command(&mut self, view: &PlayerView, legal: &LegalCommands) -> Command;
    fn answer(&mut self, view: &PlayerView, decision: &Decision) -> Answer;
}
```

Implementations: `HumanController` (fed by the client's channel), `AiController`, `ReplayController`, and later `RemoteController`.

### 15.2 AI

- **v1 is greedy one-ply.** For each legal command: clone, resolve, score, keep the best. Workers on a `rayon` pool each own a Lua state.
- **Later, MCTS with determinization**, sampling hidden tokens and dice outcomes.
- **The evaluator is modular.** Modules register terms: material (points remaining, weighted by wounds), threat (expected damage each side can deal next turn), and objective progress from each victory condition's `progress` estimate.
- **Archetypes give free priors.** An aura's value can be estimated from its descriptor ("+1 defense to adjacent soldiers") without simulating it. Script-only abilities still count, through simulation.
- **Decisions inside resolution** are answered by simulating each option to a shallow depth, falling back to the first option when the budget runs out.

### 15.3 Visibility

`PlayerView` is a per-player projection of state, and the event stream is filtered the same way. Markers and events carry a visibility of `public`, `owner` or `none`. An opponent's token placement arrives as "placed four tokens", with no assignments. The AI sees only its own view.

### 15.4 Replays

The replay viewer is a `ReplayController` plus snapshot seeking, with a toggle for each player's perspective. Any replay can be exported as a bug report, and `strata replay to-test` turns a bug report into a regression test.

### 15.5 If online play arrives

The design already supports lockstep: peers exchange commands and answers, every peer runs the core, and state hashes are compared after each command. Two additions keep it honest.

- **Dice.** Each player commits to a hash chain at game start and reveals one link per command, after the command is locked. The RNG for that command is seeded from every player's revealed link. Nobody can predict a roll before committing, and nobody can bias one, because the chains were fixed in advance.
- **Hidden tokens.** Each placement is published as a salted hash and revealed individually on its turn. Decoys are committed the same way, so the count of real tokens never leaks.

## 16. Modding workflow and tooling

The authoring loop: edit a Lua file with autocomplete, see the change in a running match within seconds, then prove it with a test. Liam builds the standard pack through this same loop, which is what keeps it good.

### 16.1 CLI (`strata`)

| Command | What it does |
| --- | --- |
| `strata pack validate <path>` | Runs the data stages and freeze; prints every error with file and line |
| `strata pack test <path>` | Runs the pack's Lua tests (16.4) |
| `strata lint <path>` | Runs `selene` plus Strata's own checks: `/` division, upvalue writes in hooks, card-text drift, sprite height against the LOS profile |
| `strata stubs` | Writes LuaLS type stubs for editor autocomplete (section 9.5) |
| `strata stats abilities <path>` | Archetype coverage (section 8.6) |
| `strata sim --format F --armies a.lua b.lua --games 1000` | AI self-play for balance: win rates, points destroyed, per-unit survival |
| `strata replay verify / dump / to-test` | Re-resolve and check hashes; print as text; turn into a regression test |

### 16.2 Hot reload

Dev builds watch every loaded pack folder with the `notify` crate. On change, the data stages rerun. If the result is valid, the registry swaps and the current match replays its log under the new definitions, up to the same command index. If a command has become illegal, the match stops at the last legal one. Load errors appear as an in-game overlay rather than a crash.

### 16.3 Editor setup

A pack template ships a `.luarc.json` pointing LuaLS at the generated stubs, plus a `selene.toml` with Strata's standard library definition. A modder opens the folder in VS Code, Neovim or Zed and gets completion, type warnings and lint on save.

### 16.4 Pack tests

Tests skip deployment, place figures directly, rig dice, and assert on queries, events and state.

```lua
test {
  name = "Shieldwall adds defense to an adjacent soldier",
  ruleset = "strata_std:advanced",
  map = "strata_std:test_flat_10",
  place = {
    { "ironclad_line", player = 0, at = { { 0, 0, 1 }, { 1, 0, 1 } } },
    { "strata_std:test_dummy", player = 1, at = { { 2, 0, 1 } } },
  },
  steps = {
    expect.stat("p0.ironclad_line.trooper#2", "defense", 5),
    act.attack("p1.test_dummy#1", "p0.ironclad_line.trooper#2", {
      rig = { { "hit", "hit", "hit" }, { "block", "blank", "blank", "blank", "blank" } },
    }),
    expect.event("figure_destroyed", { figure = "p0.ironclad_line.trooper#2" }),
  },
}
```

On the Rust side, `cargo test` covers the crates, `proptest` checks geometry invariants, and `insta` snapshots the event logs of golden matches. A good invariant: line of sight must not change when the whole board is rotated 60 degrees.

### 16.5 Determinism harness

CI runs every pack test and a batch of seeded self-play matches twice, in separate processes, and compares the state-hash sequences. Lua 5.4 seeds string hashing per state, so any script that relies on table order fails loudly here. The suite runs on Windows, macOS and Linux to catch platform drift.

### 16.6 Rules inspector

A dev panel answers "why is this number 5?" for any query. Click a stat and it shows the trace: base 4, plus 1 from Shieldwall on the adjacent figure (native), clamped to 0 to 99, result 5. Slow-path Lua calls are marked, so the inspector doubles as a performance hint.

## 17. Build order

Build bottom-up: geometry and the Lua loader before rules, and rules before the client. No milestone starts until the previous gate passes.

| Milestone | Scope | Exit gate |
| --- | --- | --- |
| M0 Foundations | Workspace, kernel geometry and RNG, model types, lints; CI on Windows, macOS and Linux | Geometry property tests pass; identical hashes on all three OSes |
| M1 Lua data stage | `mlua` sandbox, constructors, stages, freeze, registry; LuaLS stubs and `strata pack validate` | Every load error names its file and line |
| M2 Rules core | Slots, registrar, queries, descriptors, triggers, windows; answer tape, Lua module adapter, Lua pack tests | Pack tests pass with one slot swapped between Rust and Lua |
| M3 Standard rules and archetypes | Rust modules for every slot; the eight archetype families; 8 original test units in `strata_std` | 1,000 self-play games, zero hash mismatches |
| M4 Playable client | Sim thread, iso camera, terrain, sprites, playback, previews, decision UI; hotseat with placeholder visuals | A full hotseat match with zero art assets |
| M5 Map editor | Editor core, palette, level slicing, zones, symmetry; Lua writer, scenario skeletons, playtest button | Build, validate and playtest a map without leaving the app |
| M6 AI and tooling | Greedy AI, balance sim, coverage report, rules inspector, hot reload, lints, replay tools | The AI beats a random player in 90% of games; coverage report on `strata_std` |
| M7 Content and release | Units, factions and art for `strata_std`; release packaging | A distributable build |

Implementation is tracked as GitHub issues in the engine repository, starting at T-001. Each pull request adds at most 500 lines of core logic, not counting tests. Every pull request passes CI and an automated Claude review before Liam reviews it.

## 18. Open questions

- [ ] **Archetype boundaries.** Before M3, sort a broad sample of planned abilities into the eight families on paper. Do they cluster as expected, and which descriptors are missing?
- [ ] **Lua 5.4 or Luau.** Luau has a built-in type checker and a sandbox-first design, but its numbers are doubles only and it uses an interrupt rather than an instruction-count hook. Stay on 5.4 unless typing pain outweighs that.
- [ ] **Slot granularity.** Should `engagement` stay separate from `movement`? Do `units` and `deployment` earn their own slots, or fold into others?
- [ ] **Rules interpretations.** Where a rule could reasonably go either way (level spread across a two-space base, headroom under arches, edge-grazing LOS), which default does each standard module use? Each becomes a config key either way.
- [ ] **Web later.** If web becomes a goal, swap `mlua` for a pure-Rust Lua such as lua-rs. Keeping `strata_lua` the only crate that names `mlua` makes that a one-crate change.
- [ ] **Launch roster.** How many units does the standard pack need for the game to be worth playing?
- [ ] **LOS defaults.** Center-only or center-plus-corners hit-zone samples? Prototype both against a dozen hand-built cases.
