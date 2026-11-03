//! The [`RuleHost`] a pack test runs under: content lookups, native descriptors, stat queries
//! and rigged dice.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock, Weak};

use strata_content::ContentRegistry;
use strata_kernel::{DefId, Space, Sym, Value};
use strata_lua::{Def, LuaRules, RuleHost};
use strata_model::board::{BoardDefs, CompiledBoard};
use strata_model::{
    Descriptor, DieDef, DieKind, FeatureDef, FigureId, FigureStatus, GameState, MarkerDef,
    PlayerId, StatDef, TerrainDef, TileDef, UnitDef,
};
use strata_rules::{
    Ctx, EvalCtx, Halt, LogicError, LogicEvaluator, Logics, NativeEvaluator, QueryEngine,
    SourcedModifier, StatBounds, StatQuery, World,
};

/// The definition called `name`: as written, then in `pack`, then in any pack.
pub(super) fn find<'a, T: Def>(
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
pub(super) struct RegistryDefs<'a> {
    pub registry: &'a ContentRegistry,
    pub pack: &'a str,
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

pub(super) struct Host {
    registry: ContentRegistry,
    pack: String,
    modifiers: Vec<SourcedModifier>,
    bounds: BTreeMap<Sym, StatBounds>,
    board: Option<CompiledBoard>,
    rig: Mutex<Rig>,
    /// Set once the rules exist, because the rules hold this host.
    pub(super) rules: OnceLock<Weak<LuaRules>>,
}

impl Host {
    pub(super) fn new(
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
            rig: Mutex::default(),
            rules: OnceLock::new(),
        }
    }

    /// Forces the dice of the next command and counts its rolls from the first.
    pub(super) fn set_rig(&self, rolls: Vec<Vec<Sym>>) {
        *self.rig.lock().unwrap() = Rig { rolls, next: 0 };
    }

    /// Starts the rig over, because answering a decision runs the command again from the start.
    pub(super) fn rewind(&self) {
        self.rig.lock().unwrap().next = 0;
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
        let asked = rules.with_state(|scripts| {
            let logics = Logics {
                evaluator: &evaluator,
                scripts,
            };
            QueryEngine::new(&self.modifiers, &self.bounds, logics).query(&query)
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
}
