//! `core:engagement`: who is engaged with whom, and reprisals for leaving (design §6.1, §6.6).
//!
//! A figure is engaged with an enemy that covers an adjacent column with any cell of its
//! footprint, when their height ranges overlap there (`heights = "overlap"`), or whatever their
//! levels (`heights = "ignore"`). That geometric answer is the default of the `is_engaged` vote,
//! so modifiers can change it either way. Figures further apart are never engaged.
//!
//! A figure that walks out of engagement with an enemy gives that enemy one reprisal per move,
//! unless the `provokes_reprisal` vote denies it: a roll of `dice` of `die`, where every `wound_on`
//! face wounds the leaving figure through `core:units`. A multi-space body leaves an enemy only
//! when none of its cells stays engaged with it. Reprisals resolve before the move applies.

mod reprisal;

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use strata_kernel::{ModuleId, SlotId, Space, Sym, Value};
use strata_model::{ConfigField, FigureId, FigureState, GameState};
use strata_rules::{
    ConfigSchema, Ctx, Engagement, EngagementMemo, Halt, LogicError, ModuleError, NativeEnv,
    Registrar, RuleModule, VoteQuery,
};

pub use self::reprisal::REPRISAL;
use self::reprisal::Reprisal;

#[cfg(test)]
mod tests;

/// The engagement slot.
pub struct EngagementModule;

/// How the levels two adjacent figures stand on decide engagement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Heights {
    /// Engaged only when the levels each figure spans overlap.
    Overlap,
    /// Engaged whatever their levels.
    Ignore,
}

/// The service `core:engagement` provides.
struct Service {
    heights: Heights,
    reprisal: Reprisal,
}

/// The figure's ID as query subjects and contexts carry it.
fn id_value(figure: FigureId) -> Value {
    Value::Int(i64::from(figure.0))
}

/// The team of the player controlling `figure`.
fn team(state: &GameState, figure: &FigureState) -> u8 {
    state.team(figure.controller)
}

/// How many levels `figure` spans, at least one.
fn height(env: &dyn NativeEnv, figure: &FigureState) -> u32 {
    let def = env.figure_def(&figure.def);
    def.map_or(1, |d| u32::from(d.height).max(1))
}

/// Whether the level ranges `[a, a + a_height)` and `[b, b + b_height)` share a level.
pub(crate) fn overlaps(a: u32, a_height: u32, b: u32, b_height: u32) -> bool {
    a < b.saturating_add(b_height) && b < a.saturating_add(a_height)
}

impl Engagement for Service {
    fn engaged(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        figure: FigureId,
        at: Space,
    ) -> Result<Vec<FigureId>, LogicError> {
        let me = state.figures.get(figure.index());
        let me = me.ok_or_else(|| LogicError(format!("no figure {}", figure.0)))?;
        let (mine, tall) = (team(state, me), height(env, me));
        // Every covered cell of every enemy counts, so a long body engages along its length.
        let mut near: BTreeMap<FigureId, bool> = BTreeMap::new();
        for (&there, &id) in &state.board_changes.occupancy {
            let Some(other) = state.figures.get(id.index()) else {
                continue;
            };
            let enemy = id != figure && team(state, other) != mine;
            if !enemy || there.hex().distance(at.hex()) != 1 || state.standing(id).is_none() {
                continue;
            }
            let level = match self.heights {
                Heights::Overlap => overlaps(at.z, tall, there.z, height(env, other)),
                Heights::Ignore => true,
            };
            *near.entry(id).or_default() |= level;
        }
        let mut found = Vec::new();
        for (id, level) in near {
            let query = VoteQuery {
                default: level,
                ..VoteQuery::is_engaged(id_value(figure), id_value(id))
            };
            if env.vote(state, &query)? {
                found.push(id);
            }
        }
        Ok(found)
    }

    fn leaving_provokes(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        figure: FigureId,
        enemy: FigureId,
    ) -> Result<bool, LogicError> {
        // Asked only of an enemy being left, so leaving provokes unless a modifier denies it.
        let query = VoteQuery {
            default: true,
            ..VoteQuery::provokes_reprisal(id_value(figure), id_value(enemy))
        };
        env.vote(state, &query)
    }

    fn leave(
        &self,
        ctx: &mut dyn Ctx,
        env: &dyn NativeEnv,
        figure: FigureId,
        walk: &[Vec<Space>],
    ) -> Result<(), Halt> {
        if ctx.state().standing(figure).is_none() {
            return Ok(());
        }
        // Every reprisal is owed before any resolves, so the walk is judged on one state.
        let owed = EngagementMemo::new(self, ctx.state(), env).owed_along(figure, walk);
        let owed = owed.map_err(|e| Halt::reject(e.0))?;
        for enemy in owed {
            let state = ctx.state();
            if state.standing(figure).is_none() {
                break;
            }
            if state.standing(enemy).is_some() {
                self.reprisal.resolve(ctx, env, enemy, figure)?;
            }
        }
        Ok(())
    }
}

fn field(kind: &str, default: Value) -> ConfigField {
    ConfigField {
        kind: Sym::new(kind),
        default,
    }
}

impl RuleModule for EngagementModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("core:engagement")
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("engagement"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        static SCHEMA: OnceLock<ConfigSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            let text = |s: &str| field("str", Value::Str(Sym::new(s)));
            ConfigSchema::new(BTreeMap::from([
                (Sym::new("heights"), text("overlap")),
                (Sym::new("die"), text("combat")),
                (Sym::new("dice"), field("int", Value::Int(1))),
                (Sym::new("wound_on"), text("hit")),
            ]))
        })
    }

    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError> {
        let bad = |name: &str, e: &dyn std::fmt::Display| {
            ModuleError::Invalid(format!("core:engagement `{name}`: {e}"))
        };
        let text = |name: &str| {
            cfg.get(name)
                .and_then(Value::as_str)
                .map_err(|e| bad(name, &e))
        };
        let heights = match text("heights")? {
            "overlap" => Heights::Overlap,
            "ignore" => Heights::Ignore,
            other => return Err(bad("heights", &format!("unknown value `{other}`"))),
        };
        let dice = cfg.get("dice").and_then(Value::as_int);
        let dice = dice.map_err(|e| bad("dice", &e))?;
        if !(0..=strata_rules::MAX_POOL).contains(&dice) {
            let range = format!("must be 0..={}", strata_rules::MAX_POOL);
            return Err(bad("dice", &range));
        }
        let reprisal = Reprisal {
            die: text("die")?.to_owned(),
            dice,
            wound_on: Sym::new(text("wound_on")?),
        };
        let service: Arc<dyn Engagement> = Arc::new(Service { heights, reprisal });
        r.service(Box::new(service));
        Ok(())
    }
}
