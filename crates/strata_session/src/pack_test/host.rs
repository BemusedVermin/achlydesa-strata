//! The [`RuleHost`] a pack test runs under: content lookups and stat queries.

use std::collections::BTreeMap;
use std::sync::{OnceLock, Weak};

use strata_content::ContentRegistry;
use strata_kernel::{DefId, Sym, Value};
use strata_lua::{Def, LuaRules, RuleHost};
use strata_model::{Descriptor, DieDef, DieKind, FigureId, GameState, StatDef, UnitDef};
use strata_rules::{
    Ctx, EvalCtx, Halt, LogicError, LogicEvaluator, Logics, QueryEngine, SourcedModifier,
    StatBounds, StatQuery,
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

const NO_NATIVE: &str = "native descriptors are not supported by the pack test harness yet";

/// Native descriptors need the descriptor world, which a later part of the harness adds.
struct NoNative;

impl LogicEvaluator for NoNative {
    fn eval(&self, _: &Descriptor, _: &EvalCtx<'_>) -> Result<Value, LogicError> {
        Err(LogicError(NO_NATIVE.into()))
    }
}

pub(super) struct Host {
    registry: ContentRegistry,
    pack: String,
    modifiers: Vec<SourcedModifier>,
    bounds: BTreeMap<Sym, StatBounds>,
    /// Set once the rules exist, because the rules hold this host.
    pub(super) rules: OnceLock<Weak<LuaRules>>,
}

impl Host {
    pub(super) fn new(
        registry: &ContentRegistry,
        pack: &str,
        modifiers: Vec<SourcedModifier>,
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
            rules: OnceLock::new(),
        }
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
        let asked = rules.with_state(|scripts| {
            let logics = Logics {
                evaluator: &NoNative,
                scripts,
            };
            QueryEngine::new(&self.modifiers, &self.bounds, logics).query(&query)
        });
        let asked = asked.map_err(|_| fail("the script state failed".into()))?;
        asked.map_err(|e| fail(e.to_string()))
    }
}

impl RuleHost for Host {
    fn eval(&self, _: &GameState, _: &Descriptor, _: &Value) -> Result<Value, LogicError> {
        Err(LogicError(NO_NATIVE.into()))
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
