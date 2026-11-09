//! `core:victory`: how a match ends, from the scenario's `victory` list (design §6.1, §11.2).
//!
//! The module provides [`Victory`], which the engine asks to settle once every command has
//! resolved. Each condition is checked in list order and the first one met ends the match, so
//! the first listed wins ties. When the command ended a round, every condition is checked at
//! that round's end; otherwise only those that apply after any command are. The conditions the
//! `win.*` constructors write are:
//!
//! - `{ win = "eliminate_all" }`, after any command and at round end: when every team but one
//!   has no figure left in play, that team wins; when none has, the match is drawn. Only teams
//!   that fielded figures take part, and figures count for their unit's owner.
//! - `{ win = "hold_zone", zone = Z, rounds = N }`, at round end: a team holds zone `Z` when
//!   only its figures cover any of the zone's spaces. Holding it at `N` round ends in a row
//!   (default 1) wins.
//! - `{ win = "round_limit", rounds = N, tiebreak = { ... } }`, at round end: the end of round
//!   `N` ends the match. Each tiebreak in turn keeps the teams that score highest on it, and a
//!   single team left wins; otherwise the match is drawn. The tiebreaks are
//!   `points_remaining` (points of the team's units with a figure in play),
//!   `points_destroyed` (points of the other teams' units with none) and `figures_remaining`.
//!
//! A condition written as a Lua function is called with `ctx` and the moment, `"command"` or
//! `"round_end"`, and returns nil or false while it is not met, a team number for that team's
//! win, or `"draw"`.
//!
//! The end is announced by `match_ended`, carrying `{ condition, index, winners }`, and kept in
//! the variable `core:victory.result` with the same fields.

mod conditions;

use std::collections::BTreeMap;

use strata_kernel::{Frac, ModuleId, SlotId, Sym, Value};
use strata_model::{Event, GameState, Logic, PlayerId, VarKey};
use strata_rules::{
    ConfigSchema, Ctx, Halt, LogicError, MatchResult, ModuleError, NativeEnv, Progress, Registrar,
    RuleModule, Victory,
};

use crate::rounds::ROUND_ENDED;
use crate::toy::no_config;

pub use conditions::{SCRIPT_CONDITION, WIN_FIELD};

/// The custom event that announces the end of the match.
pub const MATCH_ENDED: &str = "match_ended";
/// The module that owns the variables.
const OWNER: &str = "core:victory";

/// The victory slot.
pub struct VictoryModule;

impl RuleModule for VictoryModule {
    fn id(&self) -> ModuleId {
        ModuleId::new(OWNER)
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("victory"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        no_config()
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.service::<dyn Victory>(Box::new(Service));
        Ok(())
    }
}

/// A variable of this module.
pub(crate) fn key(name: &str) -> VarKey {
    VarKey {
        module: ModuleId::new(OWNER),
        name: Sym::new(name),
    }
}

/// When the conditions are being checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Moment {
    /// After a command that ended no round.
    Command,
    /// At the end of this round.
    RoundEnd(u32),
}

/// What a met condition decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// This team won.
    Team(u8),
    /// Nobody won.
    Draw,
}

/// The rounds `events` ended, in order.
fn rounds_ended(events: &[Event]) -> Vec<u32> {
    let ended = events.iter().filter_map(|e| match e {
        Event::Custom { id, payload } if id.as_str() == ROUND_ENDED => payload.get("round").ok(),
        _ => None,
    });
    ended
        .filter_map(|round| round.as_int().ok())
        .map(|round| round as u32)
        .collect()
}

/// The service `core:victory` provides.
struct Service;

impl Victory for Service {
    fn settle(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, events: &[Event]) -> Result<(), Halt> {
        let ended = rounds_ended(events);
        let moments = if ended.is_empty() {
            vec![Moment::Command]
        } else {
            ended.into_iter().map(Moment::RoundEnd).collect()
        };
        for moment in moments {
            if self.result(ctx.state()).is_some() {
                break;
            }
            check(ctx, env, moment)?;
        }
        Ok(())
    }

    fn result(&self, state: &GameState) -> Option<MatchResult> {
        let found = state.vars.get(&key("result"))?;
        let condition = Sym::new(found.get("condition").ok()?.as_str().ok()?);
        let index = found.get("index").ok()?.as_int().ok()? as usize;
        let winners = found.get("winners").ok()?.as_list().ok()?;
        let winners = winners.iter().filter_map(|w| w.as_int().ok());
        Some(MatchResult {
            condition,
            index,
            winners: winners.map(|w| PlayerId(w as u8)).collect(),
        })
    }

    fn progress(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        player: PlayerId,
    ) -> Result<Vec<Progress>, LogicError> {
        let Some(scenario) = env.scenario(state) else {
            return Ok(Vec::new());
        };
        let mut found = Vec::new();
        for (index, logic) in scenario.victory.iter().enumerate() {
            let Logic::Native(descriptor) = logic else {
                continue;
            };
            let value = conditions::progress(state, &descriptor.0, index, state.team(player))?;
            if let Some(value) = value {
                found.push(Progress {
                    condition: conditions::name(logic),
                    index,
                    value,
                });
            }
        }
        Ok(found)
    }
}

/// Checks every condition at `moment` in list order and ends the match on the first one met.
fn check(ctx: &mut dyn Ctx, env: &dyn NativeEnv, moment: Moment) -> Result<(), Halt> {
    let Some(scenario) = env.scenario(ctx.state()) else {
        return Ok(());
    };
    for (index, logic) in scenario.victory.iter().enumerate() {
        let verdict = match logic {
            Logic::Native(d) => conditions::native(ctx, env, &d.0, index, moment)?,
            Logic::Script(_) => conditions::script(ctx, env, logic, moment)?,
        };
        if let Some(verdict) = verdict {
            return end(ctx, conditions::name(logic), index, verdict);
        }
    }
    Ok(())
}

/// Records the result and announces it.
fn end(ctx: &mut dyn Ctx, condition: Sym, index: usize, verdict: Verdict) -> Result<(), Halt> {
    let state = ctx.state();
    let winners = (0u8..).zip(&state.players);
    let winners = winners.filter(|(_, p)| verdict == Verdict::Team(p.team));
    let winners = winners.map(|(id, _)| Value::Int(i64::from(id)));
    let result = Value::Map(BTreeMap::from([
        (Sym::new("condition"), Value::Str(condition)),
        (Sym::new("index"), Value::Int(index as i64)),
        (Sym::new("winners"), Value::List(winners.collect())),
    ]));
    ctx.emit(Event::VarSet {
        key: key("result"),
        value: result.clone(),
    })?;
    ctx.emit(Event::Custom {
        id: Sym::new(MATCH_ENDED),
        payload: result,
    })
}

/// `part / whole` as a fraction of one, or `None` when `whole` is zero.
pub(crate) fn share(part: usize, whole: usize) -> Option<Frac> {
    (whole > 0).then(|| Frac::new(part.min(whole) as i128, whole as i128))
}

#[cfg(test)]
mod tests;
