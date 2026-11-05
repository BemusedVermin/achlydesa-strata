//! `core:engagement`: who is engaged with whom, and reprisals for leaving (design §6.1, §6.6).
//!
//! A figure is engaged with an enemy in an adjacent column when their height ranges overlap
//! (`heights = "overlap"`), or whatever their levels (`heights = "ignore"`). That geometric answer
//! is the default of the `is_engaged` vote, so modifiers can change it either way. Figures further
//! apart are never engaged.
//!
//! A figure that walks out of engagement with an enemy gives that enemy one reprisal per move,
//! unless the `provokes_reprisal` vote denies it: a roll of `dice` of `die`, where every `wound_on`
//! face wounds the leaving figure through `core:units`. Reprisals resolve before the move applies.

mod reprisal;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use strata_kernel::{ModuleId, SlotId, Space, Sym, Value};
use strata_model::{ConfigField, FigureId, FigureState, FigureStatus, GameState};
use strata_rules::{
    ConfigSchema, Ctx, Engagement, Halt, LogicError, ModuleError, NativeEnv, Registrar, RuleModule,
    VoteQuery,
};

pub use self::reprisal::REPRISAL;
use self::reprisal::Reprisal;

#[cfg(test)]
mod tests;

/// The engagement slot.
pub struct EngagementModule;

/// How the levels two adjacent figures stand on decide engagement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heights {
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
    let player = state.players.get(figure.controller.index());
    player.map_or(figure.controller.0, |p| p.team)
}

/// How many levels `figure` spans, at least one.
fn height(env: &dyn NativeEnv, figure: &FigureState) -> u32 {
    let def = env.figure_def(&figure.def);
    def.map_or(1, |d| u32::from(d.height).max(1))
}

/// Where `figure` stands, if it is on the board.
pub(crate) fn standing(state: &GameState, figure: FigureId) -> Option<Space> {
    let fig = state.figures.get(figure.index())?;
    fig.location.filter(|_| fig.status == FigureStatus::OnBoard)
}

/// Whether the level ranges `[a, a + a_height)` and `[b, b + b_height)` share a level.
pub fn overlaps(a: u32, a_height: u32, b: u32, b_height: u32) -> bool {
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
        let mut found = Vec::new();
        for (index, other) in (0u32..).zip(&state.figures) {
            let id = FigureId(index);
            let Some(there) = standing(state, id).filter(|_| id != figure) else {
                continue;
            };
            if team(state, other) == mine || there.hex().distance(at.hex()) != 1 {
                continue;
            }
            let level = match self.heights {
                Heights::Overlap => overlaps(at.z, tall, there.z, height(env, other)),
                Heights::Ignore => true,
            };
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

    fn reprisals(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        figure: FigureId,
        from: Space,
        to: Space,
    ) -> Result<Vec<FigureId>, LogicError> {
        let after = self.engaged(state, env, figure, to)?;
        let mut owed = Vec::new();
        for enemy in self.engaged(state, env, figure, from)? {
            if after.contains(&enemy) {
                continue;
            }
            // Asked only of an enemy being left, so leaving provokes unless a modifier denies it.
            let query = VoteQuery {
                default: true,
                ..VoteQuery::provokes_reprisal(id_value(figure), id_value(enemy))
            };
            if env.vote(state, &query)? {
                owed.push(enemy);
            }
        }
        Ok(owed)
    }

    fn leave(
        &self,
        ctx: &mut dyn Ctx,
        env: &dyn NativeEnv,
        figure: FigureId,
        path: &[Space],
    ) -> Result<(), Halt> {
        let Some(mut at) = standing(ctx.state(), figure) else {
            return Ok(());
        };
        let mut owed: Vec<FigureId> = Vec::new();
        for &next in path {
            let found = self.reprisals(ctx.state(), env, figure, at, next);
            for enemy in found.map_err(|e| Halt::reject(e.0))? {
                if !owed.contains(&enemy) {
                    owed.push(enemy);
                }
            }
            at = next;
        }
        for enemy in owed {
            let state = ctx.state();
            if standing(state, figure).is_none() {
                break;
            }
            if standing(state, enemy).is_some() {
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
            return Err(bad("dice", &"must be 0..=1024"));
        }
        let reprisal = Reprisal {
            die: text("die")?.to_owned(),
            dice,
            wound_on: Sym::new(text("wound_on")?),
        };
        r.service::<dyn Engagement>(Box::new(Service { heights, reprisal }));
        Ok(())
    }
}
