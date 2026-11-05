//! The [`RuleHost`] a pack test runs under: content lookups, native descriptors, stat queries
//! and rigged dice.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::cards::{AbilityLine, CardSource, FigureLine, StatLine, UnitCard};
use crate::names::Names;
use strata_content::ContentRegistry;
use strata_kernel::{Blocker, DefId, Hex, SUB_LEVELS, Space, Sym, Value};
use strata_lua::{Def, LuaRules, RuleHost};
use strata_model::board::CompiledBoard;
use strata_model::{
    AbilityDef, AbilityOwner, Descriptor, DieDef, DieKind, FigureId, FigureStatus, FootprintDef,
    GameState, MarkerDef, PlayerId, RelicDef, StatDef, TerrainDef, UnitDef, UnitId, UnitState,
};
use strata_rules::{
    Ctx, EvalCtx, FigureVolume, Halt, LineOfSight, LogicError, LogicEvaluator, Logics, LosWorld,
    NativeEvaluator, QueryEngine, SourcedModifier, StatBounds, StatQuery, World,
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
    registry.lookup::<T>(pack, name)
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
        let bounds = registry.iter::<StatDef>().flat_map(|(_, id, def)| {
            let bare = id.split_once(':').map_or(id, |(_, name)| name);
            let bounds = StatBounds::from(def);
            [(Sym::new(id), bounds), (Sym::new(bare), bounds)]
        });
        Self {
            registry: registry.clone(),
            pack: pack.to_owned(),
            modifiers,
            bounds: bounds.collect(),
            board,
            los: None,
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
            let queried = figure.and_then(|f| self.stat(state, *f, bare).ok());
            let label = stat_def.abbrev.as_ref().unwrap_or(&stat_def.name);
            let line = StatLine {
                label: label.0.clone(),
                base,
                value: queried.unwrap_or(base),
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

    /// Forces the dice of the next command and counts its rolls from the first.
    pub(super) fn set_rig(&self, rolls: Vec<Vec<Sym>>) {
        *self.rig.lock().unwrap() = Rig { rolls, next: 0 };
    }

    /// Starts the rig over, because answering a decision runs the command again from the start.
    pub(super) fn rewind(&self) {
        self.rig.lock().unwrap().next = 0;
    }

    /// The modifiers of attached abilities that reach `figure`: those owned by its controller,
    /// its unit, or both. A relic's grant is named by its relic and `grant` index.
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
            let grant = instance.params.get("grant").and_then(|v| v.as_int().ok());
            let named = match grant {
                Some(n) => {
                    let relic = self.registry.get::<RelicDef>(instance.def);
                    let relic = relic.zip(self.registry.name::<RelicDef>(instance.def));
                    relic.and_then(|(r, name)| {
                        let abilities = &r.grants.as_ref()?.abilities;
                        Some((abilities.get(usize::try_from(n).ok()?)?, name))
                    })
                }
                None => {
                    let ability = self.registry.get::<AbilityDef>(instance.def);
                    ability.zip(self.registry.name::<AbilityDef>(instance.def))
                }
            };
            let Some((ability, name)) = named.filter(|_| reaches) else {
                continue;
            };
            let modifiers = ability.modifiers.iter();
            found.extend(modifiers.map(|m| SourcedModifier {
                source: Sym::new(name),
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
            QueryEngine::new(&modifiers, &self.bounds, logics).query(&query)
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

    fn die(&self, name: &str) -> Option<(DefId, DieKind)> {
        let (id, def) = find::<DieDef>(&self.registry, &self.pack, name)?;
        Some((id, def.kind.clone()))
    }

    fn marker(&self, name: &str) -> Option<DefId> {
        find::<MarkerDef>(&self.registry, &self.pack, name).map(|(id, _)| id)
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
