//! The [`RuleHost`] a pack test runs under: content lookups, native descriptors, stat queries
//! and rigged dice.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::cards::{AbilityLine, CardSource, FigureLine, StatLine, UnitCard};
use crate::names::Names;
use strata_content::ContentRegistry;
use strata_kernel::{Blocker, DefId, Hex, SUB_LEVELS, Space, Sym, Value};
use strata_lua::{Def, LuaRules, RuleHost};
use strata_model::board::{BoardDefs, CompiledBoard};
use strata_model::{
    AbilityOwner, Descriptor, DieDef, DieKind, FeatureDef, FigureId, FigureStatus, FootprintDef,
    GameState, MapDef, MarkerDef, PlayerId, RelicDef, ScenarioDef, StatDef, TerrainDef, TileDef,
    UnitDef, UnitId, UnitState,
};
use strata_rules::{
    Ctx, DeploySlot, EvalCtx, FigureVolume, Halt, LineOfSight, LogicError, LogicEvaluator, Logics,
    LosWorld, MoveWorld, Movement, NativeEvaluator, Query, QueryEngine, QueryTrace, Reach,
    SourcedModifier, StatBounds, StatQuery, TerrainRules, VoteQuery, World,
};

impl CardSource for Host {
    fn names(&self) -> Arc<Names> {
        self.names.get().cloned().unwrap_or_default()
    }

    fn cards(&self, state: &GameState) -> Vec<UnitCard> {
        let units = (0u32..).zip(&state.units);
        let cards = units.map(|(id, unit)| self.card(state, UnitId(id), unit));
        cards.flatten().collect()
    }
}

/// The definition called `name`: as written, then in `pack`, then in any pack.
pub(crate) fn find<'a, T: Def>(
    registry: &'a ContentRegistry,
    pack: &str,
    name: &str,
) -> Option<(DefId, &'a T)> {
    let suffix = format!(":{name}");
    registry
        .find::<T>(name)
        .or_else(|| registry.find::<T>(&format!("{pack}:{name}")))
        .or_else(|| {
            let mut hits = registry
                .iter::<T>()
                .filter(|(_, id, _)| id.ends_with(&suffix));
            hits.next().map(|(id, _, def)| (id, def))
        })
}

/// Resolves the tile and feature names of a map against the registry.
pub(crate) struct RegistryDefs<'a> {
    pub(crate) registry: &'a ContentRegistry,
    pub(crate) pack: &'a str,
}

impl BoardDefs for RegistryDefs<'_> {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        find::<TileDef>(self.registry, self.pack, name.as_str()).map(|(_, def)| def)
    }

    fn feature(&self, name: &Sym) -> Option<&FeatureDef> {
        find::<FeatureDef>(self.registry, self.pack, name.as_str()).map(|(_, def)| def)
    }
}

/// The faces a command's dice are forced to, one list per roll.
#[derive(Default)]
struct Rig {
    rolls: Vec<Vec<Sym>>,
    next: usize,
}

pub(crate) struct Host {
    registry: ContentRegistry,
    pack: String,
    modifiers: Vec<SourcedModifier>,
    bounds: BTreeMap<Sym, StatBounds>,
    board: Option<CompiledBoard>,
    los: Option<Arc<dyn LineOfSight>>,
    movement: Option<Arc<dyn Movement>>,
    rig: Mutex<Rig>,
    /// Set once the ruleset's phases are known.
    pub(crate) names: OnceLock<Arc<Names>>,
    /// Set once the rules exist, because the rules hold this host.
    pub(crate) rules: OnceLock<Weak<LuaRules>>,
}

impl Host {
    pub(crate) fn new(
        registry: &ContentRegistry,
        pack: &str,
        modifiers: Vec<SourcedModifier>,
        board: Option<CompiledBoard>,
    ) -> Self {
        // Entering a space always costs at least one, whether or not a pack declares the stat.
        let cost = (Sym::new(MOVE_COST), StatBounds { min: 1, max: 99 });
        let bounds = registry.iter::<StatDef>().flat_map(|(_, id, def)| {
            let bare = id.split_once(':').map_or(id, |(_, name)| name);
            let bounds = StatBounds::from(def);
            [(Sym::new(id), bounds), (Sym::new(bare), bounds)]
        });
        Self {
            registry: registry.clone(),
            pack: pack.to_owned(),
            modifiers,
            bounds: std::iter::once(cost).chain(bounds).collect(),
            board,
            los: None,
            movement: None,
            rig: Mutex::default(),
            names: OnceLock::new(),
            rules: OnceLock::new(),
        }
    }

    /// The card of one unit. Its stats are queried on the first figure still in play, and fall
    /// back to the printed values when there is none or the query fails.
    fn card(&self, state: &GameState, id: UnitId, unit: &UnitState) -> Option<UnitCard> {
        let def = self.registry.get::<UnitDef>(unit.def)?;
        let alive = |f: &FigureId| {
            let figure = state.figures.get(f.index());
            let live = [FigureStatus::OnBoard, FigureStatus::Reserve];
            figure.is_some_and(|f| live.contains(&f.status))
        };
        let wound = find::<MarkerDef>(&self.registry, &self.pack, "wound").map(|(id, _)| id);
        let figure_line = |f: &FigureId| {
            let markers = state.figures.get(f.index()).map(|s| s.markers.as_slice());
            let wounds = markers.unwrap_or_default().iter();
            FigureLine {
                figure: *f,
                alive: alive(f),
                wounds: wounds
                    .filter(|m| Some(m.def) == wound)
                    .map(|m| m.count)
                    .sum(),
            }
        };
        let figure = unit.figures.iter().find(|f| alive(f));
        let mut stats = Vec::new();
        for (stat, printed) in &def.stats {
            let (name, stat_def) = (
                self.registry.name::<StatDef>(*stat)?,
                self.registry.get::<StatDef>(*stat)?,
            );
            let base = i64::from(*printed);
            let bare = name.split_once(':').map_or(name, |(_, bare)| bare);
            let queried = figure.and_then(|f| self.stat_trace(state, *f, bare).ok());
            let label = stat_def.abbrev.as_ref().unwrap_or(&stat_def.name);
            let line = StatLine {
                label: label.0.clone(),
                base,
                value: queried.as_ref().map_or(base, |(value, _)| *value),
                trace: queried.map(|(_, trace)| trace),
            };
            stats.push((stat_def.order, line));
        }
        stats.sort_by_key(|(order, _)| *order);
        let ability = |a: &strata_model::AbilityDef| AbilityLine {
            name: a.name.0.clone(),
            text: a.text.0.clone(),
        };
        Some(UnitCard {
            unit: id,
            name: def.name.0.clone(),
            kind: def.kind.as_str().to_owned(),
            stats: stats.into_iter().map(|(_, line)| line).collect(),
            abilities: def.abilities.iter().map(ability).collect(),
            figures: unit.figures.iter().map(figure_line).collect(),
        })
    }

    /// Answers `in_los` selectors with the ruleset's line-of-sight service.
    pub(crate) fn with_los(mut self, los: Option<Arc<dyn LineOfSight>>) -> Self {
        self.los = los;
        self
    }

    /// Answers `ctx.movement` with the ruleset's movement service.
    pub(crate) fn with_movement(mut self, movement: Option<Arc<dyn Movement>>) -> Self {
        self.movement = movement;
        self
    }

    /// Forces the dice of the next command and counts its rolls from the first.
    pub(super) fn set_rig(&self, rolls: Vec<Vec<Sym>>) {
        *self.rig.lock().unwrap() = Rig { rolls, next: 0 };
    }

    /// Starts the rig over, because answering a decision runs the command again from the start.
    pub(super) fn rewind(&self) {
        self.rig.lock().unwrap().next = 0;
    }

    /// The modifiers of attached abilities that reach `figure`: those owned by its controller,
    /// its unit, or both.
    fn granted(&self, state: &GameState, figure: FigureId) -> Vec<SourcedModifier> {
        let Some(fig) = state.figures.get(figure.index()) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for instance in &state.abilities {
            let reaches = match instance.owner {
                AbilityOwner::Player(p) => p == fig.controller,
                AbilityOwner::Unit(u) => u == fig.unit,
                AbilityOwner::Relic(_) => false,
            };
            let named = reaches.then(|| self.registry.ability_with_name(instance.def));
            let Some((ability, name)) = named.flatten() else {
                continue;
            };
            let modifiers = ability.modifiers.iter();
            found.extend(modifiers.map(|m| SourcedModifier {
                source: Sym::new(&name),
                priority: 0,
                def: m.clone(),
            }));
        }
        found
    }

    /// A figure's stat after every modifier.
    pub(super) fn stat(
        &self,
        state: &GameState,
        figure: FigureId,
        stat: &str,
    ) -> Result<i64, LogicError> {
        self.stat_trace(state, figure, stat).map(|(value, _)| value)
    }

    /// A figure's stat after every modifier, with how it was reached.
    fn stat_trace(
        &self,
        state: &GameState,
        figure: FigureId,
        stat: &str,
    ) -> Result<(i64, QueryTrace), LogicError> {
        let fail = LogicError;
        let (stat_id, _) = find::<StatDef>(&self.registry, &self.pack, stat)
            .ok_or_else(|| fail(format!("unknown stat `{stat}`")))?;
        let unit = state.figures.get(figure.index()).map(|f| f.def.unit);
        let unit = unit.and_then(|id| self.registry.get::<UnitDef>(id));
        let unit = unit.ok_or_else(|| fail(format!("figure {} has no unit", figure.0)))?;
        let query = StatQuery {
            stat: Sym::new(stat),
            base: i64::from(unit.stats.get(&stat_id).copied().unwrap_or(0)),
            subject: Value::Int(i64::from(figure.0)),
            context: Value::Nil,
        };
        self.ask_trace(state, figure, &query)
    }

    /// Resolves `query` against the state with every modifier applied.
    fn ask<Q: Query>(
        &self,
        state: &GameState,
        figure: FigureId,
        query: &Q,
    ) -> Result<Q::Output, LogicError> {
        self.ask_trace(state, figure, query)
            .map(|(output, _)| output)
    }

    /// Resolves `query` like [`Host::ask`], with how it was reached.
    fn ask_trace<Q: Query>(
        &self,
        state: &GameState,
        figure: FigureId,
        query: &Q,
    ) -> Result<(Q::Output, QueryTrace), LogicError> {
        let fail = LogicError;
        let rules = self.rules.get().and_then(Weak::upgrade);
        let rules = rules.ok_or_else(|| fail("the rules are not running".into()))?;
        let world = StateWorld { host: self, state };
        let evaluator = NativeEvaluator { world: &world };
        let mut modifiers = self.modifiers.clone();
        modifiers.extend(self.granted(state, figure));
        let asked = rules.with_state(|scripts| {
            let logics = Logics {
                evaluator: &evaluator,
                scripts,
            };
            QueryEngine::new(&modifiers, &self.bounds, logics).trace(query)
        });
        let asked = asked.map_err(|_| fail("the script state failed".into()))?;
        asked.map_err(|e| fail(e.to_string()))
    }
}

impl RuleHost for Host {
    fn eval(
        &self,
        state: &GameState,
        descriptor: &Descriptor,
        subject: &Value,
    ) -> Result<Value, LogicError> {
        let world = StateWorld { host: self, state };
        let source = Sym::new("test");
        let cx = EvalCtx {
            source: &source,
            subject,
            context: &Value::Nil,
        };
        NativeEvaluator { world: &world }.eval(descriptor, &cx)
    }

    fn query(&self, state: &GameState, name: &str, args: &[Value]) -> Result<Value, LogicError> {
        let asked = |e: strata_kernel::ValueError| LogicError(format!("query `{name}`: {e}"));
        if name != "stat" || args.len() != 2 {
            return Err(LogicError(format!(
                "the harness cannot answer query `{name}`"
            )));
        }
        let figure = FigureId(args[0].as_int().map_err(asked)? as u32);
        let stat = args[1].as_str().map_err(asked)?;
        self.stat(state, figure, stat).map(Value::Int)
    }

    fn reachable(&self, state: &GameState, figure: FigureId) -> Result<Vec<Reach>, LogicError> {
        let service = self.movement.as_ref();
        let service =
            service.ok_or_else(|| LogicError("the ruleset has no `movement` module".into()))?;
        let board = self.board.as_ref();
        let board = board.ok_or_else(|| LogicError("the match has no board".into()))?;
        service.reachable(
            &StateMove {
                host: self,
                state,
                board,
            },
            figure,
        )
    }

    fn die(&self, name: &str) -> Option<(DefId, DieKind)> {
        let (id, def) = find::<DieDef>(&self.registry, &self.pack, name)?;
        Some((id, def.kind.clone()))
    }

    fn marker(&self, name: &str) -> Option<DefId> {
        find::<MarkerDef>(&self.registry, &self.pack, name).map(|(id, _)| id)
    }

    fn deployment(&self, state: &GameState, player: PlayerId) -> Option<DeploySlot> {
        let setup = &state.setup;
        let scenario = self.registry.get::<ScenarioDef>(setup.scenario)?;
        let seat = setup.players.get(player.index())?;
        let slot = scenario.slots.iter().find(|s| s.id == seat.slot)?;
        let (_, map) = find::<MapDef>(&self.registry, &self.pack, scenario.map.as_str())?;
        Some(DeploySlot {
            order: slot.deploy_order,
            zone: map.zones.get(&slot.start)?.clone(),
        })
    }

    fn standable(&self, space: Space) -> bool {
        let surfaces = self.board.as_ref().map(|b| b.surfaces(space.hex()));
        let surface = surfaces.and_then(|all| all.iter().find(|s| s.z == space.z));
        let terrain = surface.and_then(|s| self.registry.get::<TerrainDef>(s.terrain));
        terrain.is_some_and(|t| t.standable)
    }

    fn relic(&self, id: DefId) -> Option<RelicDef> {
        self.registry.get::<RelicDef>(id).cloned()
    }

    fn forced_roll(&self, die: &str, count: usize) -> Option<Result<Vec<u32>, String>> {
        let mut rig = self.rig.lock().unwrap();
        if rig.rolls.is_empty() {
            return None;
        }
        let Some(faces) = rig.rolls.get(rig.next).cloned() else {
            return Some(Err(format!("the rig has no faces left for `{die}`")));
        };
        rig.next += 1;
        if faces.len() != count {
            let got = faces.len();
            return Some(Err(format!("rigged {got} faces for a roll of {count}")));
        }
        let Some((_, DieKind::Faces(names))) = self.die(die) else {
            return Some(Err(format!("`{die}` has no faces to rig")));
        };
        let index = |face: &Sym| {
            let at = names.iter().position(|n| n == face);
            at.map(|i| i as u32 + 1)
                .ok_or_else(|| format!("`{die}` has no face `{face}`"))
        };
        Some(faces.iter().map(index).collect())
    }

    fn call(
        &self,
        namespace: &str,
        name: &str,
        _: &[Value],
        _: &mut dyn Ctx,
    ) -> Result<Value, Halt> {
        Err(Halt::reject(format!(
            "the harness has no `ctx.{namespace}.{name}`"
        )))
    }
}

/// The game as descriptors see it: the state, with units and terrain from the registry.
struct StateWorld<'a> {
    host: &'a Host,
    state: &'a GameState,
}

impl StateWorld<'_> {
    fn unit(&self, figure: FigureId) -> Option<&UnitDef> {
        let figure = self.state.figures.get(figure.index())?;
        self.host.registry.get::<UnitDef>(figure.def.unit)
    }
}

impl World for StateWorld<'_> {
    fn figures(&self) -> Vec<FigureId> {
        let all = (0u32..).zip(&self.state.figures);
        let on_board = all.filter(|(_, f)| f.status == FigureStatus::OnBoard);
        on_board.map(|(id, _)| FigureId(id)).collect()
    }

    fn location(&self, figure: FigureId) -> Option<Space> {
        self.state.figures.get(figure.index())?.location
    }

    fn controller(&self, figure: FigureId) -> PlayerId {
        self.state.figures[figure.index()].controller
    }

    fn team(&self, player: PlayerId) -> u8 {
        let player_state = self.state.players.get(player.index());
        player_state.map_or(player.0, |p| p.team)
    }

    fn has_trait(&self, figure: FigureId, value: &str) -> bool {
        self.unit(figure)
            .is_some_and(|u| u.traits.values().any(|v| v.as_str() == value))
    }

    fn kind(&self, figure: FigureId) -> Sym {
        self.unit(figure)
            .map(|u| u.kind.clone())
            .unwrap_or_default()
    }

    fn stat(&self, figure: FigureId, stat: &str) -> Result<i64, LogicError> {
        self.host.stat(self.state, figure, stat)
    }

    fn terrain_at(&self, space: Space) -> Option<Sym> {
        let surfaces = self.host.board.as_ref()?.surfaces(space.hex());
        let surface = surfaces.iter().find(|s| s.z == space.z)?;
        let name = self.host.registry.name::<TerrainDef>(surface.terrain)?;
        Some(Sym::new(
            name.split_once(':').map_or(name, |(_, bare)| bare),
        ))
    }

    fn occupant(&self, space: Space) -> Option<FigureId> {
        self.state.board_changes.occupancy.get(&space).copied()
    }

    fn phase(&self) -> Sym {
        Sym::default()
    }

    fn in_los(&self, from: FigureId, to: FigureId) -> Result<bool, LogicError> {
        let service = self.host.los.as_ref();
        let service =
            service.ok_or_else(|| LogicError("the ruleset has no `los` module".into()))?;
        let world = StateLos {
            host: self.host,
            state: self.state,
        };
        service.query(&world, from, to).map(|seen| seen.visible)
    }
}

/// The board and figure volumes line of sight reads: the state, with footprints and profiles
/// from the registry.
struct StateLos<'a> {
    host: &'a Host,
    state: &'a GameState,
}

impl LosWorld for StateLos<'_> {
    fn blockers(&self, hex: Hex) -> &[Blocker] {
        self.host.board.as_ref().map_or(&[], |b| b.blockers(hex))
    }

    fn figures(&self) -> Vec<FigureId> {
        let all = (0u32..).zip(&self.state.figures);
        let on_board = all.filter(|(_, f)| f.status == FigureStatus::OnBoard);
        on_board.map(|(id, _)| FigureId(id)).collect()
    }

    fn volume(&self, figure: FigureId) -> Option<FigureVolume> {
        let fig = self.state.figures.get(figure.index())?;
        let at = fig
            .location
            .filter(|_| fig.status == FigureStatus::OnBoard)?;
        let registry = &self.host.registry;
        let unit = registry.get::<UnitDef>(fig.def.unit)?;
        let entry = unit.figures.iter().find(|f| f.id == fig.def.figure)?;
        let footprint = registry.get::<FootprintDef>(entry.footprint)?;
        let shape = footprint.cells.rotated(fig.rotation).translated(at.hex());
        let floor = i64::from(at.z) * SUB_LEVELS;
        let height = |sub: u16| floor + i64::from(sub);
        Some(FigureVolume {
            anchor: at.hex(),
            cells: shape.cells().to_vec(),
            target: height(entry.los.target),
            bottom: height(entry.los.hit_zone.0),
            top: height(entry.los.hit_zone.1),
        })
    }
}

/// The stat that prices entering a space.
const MOVE_COST: &str = "move_cost";
/// The stat that is a figure's allowance for one move.
const MOVE: &str = "move";

/// What movement reads: the state and board, with terrain and figure sizes from the registry.
struct StateMove<'a> {
    host: &'a Host,
    state: &'a GameState,
    board: &'a CompiledBoard,
}

impl StateMove<'_> {
    /// The terrain's name as modifiers see it, without the pack prefix.
    fn terrain_name(&self, terrain: DefId) -> Sym {
        let name = self.host.registry.name::<TerrainDef>(terrain).unwrap_or("");
        Sym::new(name.split_once(':').map_or(name, |(_, bare)| bare))
    }

    /// The `{ terrain, dz }` context of a step.
    fn step(&self, terrain: DefId, dz: i32) -> Value {
        Value::Map(BTreeMap::from([
            (Sym::new("terrain"), Value::Str(self.terrain_name(terrain))),
            (Sym::new("dz"), Value::Int(i64::from(dz))),
        ]))
    }
}

impl MoveWorld for StateMove<'_> {
    fn board(&self) -> &CompiledBoard {
        self.board
    }

    fn location(&self, figure: FigureId) -> Option<Space> {
        let fig = self.state.figures.get(figure.index())?;
        fig.location.filter(|_| fig.status == FigureStatus::OnBoard)
    }

    fn occupant(&self, space: Space) -> Option<FigureId> {
        self.state.board_changes.occupancy.get(&space).copied()
    }

    fn height(&self, figure: FigureId) -> u32 {
        let fig = self.state.figures.get(figure.index());
        let unit = fig.and_then(|f| self.host.registry.get::<UnitDef>(f.def.unit));
        let entry = unit
            .zip(fig)
            .and_then(|(u, f)| u.figures.iter().find(|e| e.id == f.def.figure));
        entry.map_or(1, |e| u32::from(e.height))
    }

    fn allowance(&self, figure: FigureId) -> Result<i64, LogicError> {
        self.host.stat(self.state, figure, MOVE)
    }

    fn terrain(&self, terrain: DefId) -> TerrainRules {
        let def = self.host.registry.get::<TerrainDef>(terrain);
        let tagged = |tag: &str| def.is_some_and(|d| d.tags.iter().any(|t| t.as_str() == tag));
        TerrainRules {
            passable: def.is_some_and(|d| d.standable) && !tagged("impassable"),
            stops_here: tagged("stops_here"),
        }
    }

    fn move_cost(&self, figure: FigureId, terrain: DefId, dz: i32) -> Result<i64, LogicError> {
        let query = StatQuery {
            stat: Sym::new(MOVE_COST),
            base: 1,
            subject: Value::Int(i64::from(figure.0)),
            context: self.step(terrain, dz),
        };
        self.host.ask(self.state, figure, &query)
    }

    fn can_climb(&self, figure: FigureId, terrain: DefId, dz: i32) -> Result<bool, LogicError> {
        let subject = Value::Int(i64::from(figure.0));
        self.host.ask(
            self.state,
            figure,
            &VoteQuery::can_climb(subject, self.step(terrain, dz)),
        )
    }

    fn can_enter(&self, figure: FigureId, to: Space) -> Result<bool, LogicError> {
        let subject = Value::Int(i64::from(figure.0));
        let at = Value::List(
            [to.q, to.r, to.z as i32]
                .map(|n| Value::Int(i64::from(n)))
                .to_vec(),
        );
        self.host
            .ask(self.state, figure, &VoteQuery::can_enter(subject, at))
    }
}
