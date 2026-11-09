//! `core:victory`: how a match ends, from the scenario's `victory` list (design §6.1, §11.2).
//!
//! The module provides [`Victory`]. The session has it check the list when a match is set up,
//! so an unknown condition or tiebreak, a malformed field or a zone the map lacks is a setup
//! error. The engine asks it to settle once every command has resolved: the conditions are
//! checked after the command, then again at the end of each round the command ended, as §11.2
//! says ("after every command and at round end"). At each moment every condition is checked in
//! list order and the first one met ends the match, so the first listed wins ties. The
//! conditions the `win.*` constructors write are:
//!
//! - `{ win = "eliminate_all" }`, after any command and at round end: when every team but one
//!   has no figure left in play, that team wins; when none has, the match is drawn. Only teams
//!   that fielded figures take part, and figures count for their unit's owner, since
//!   elimination is about whose army is gone.
//! - `{ win = "hold_zone", zone = Z, rounds = N }`, at round end: a team holds zone `Z` when
//!   only its figures cover any of the zone's spaces, each counting for the team that controls
//!   it, since holding is about who stands there. Holding it at `N` round ends in a row
//!   (default 1) wins; a round end the team does not hold it, or another team does, starts
//!   the count again.
//! - `{ win = "round_limit", rounds = N, tiebreak = { ... } }`, at round end: the end of round
//!   `N` ends the match. Each tiebreak in turn keeps the teams that score highest on it, and a
//!   single team left wins; otherwise the match is drawn. The tiebreaks, which count units for
//!   their owner's team, are `points_remaining` (printed points of the team's units with a
//!   figure in play), `points_destroyed` (printed points of the other teams' units with none)
//!   and `figures_remaining`. Points are the printed `points` stat, as `core:army` prices
//!   units; a unit without one, or a pack without the stat, counts 0.
//!
//! A condition written as a Lua function is called with `ctx` and the moment, `"command"` or
//! `"round_end"`, and returns nil or false while it is not met, a team number for that team's
//! win, or `"draw"`.
//!
//! A condition that fails while the match is played, such as a Lua function that raises an
//! error or returns something else, never refuses the player's command: it is skipped at that
//! moment and `victory_condition_failed` reports it, carrying `{ condition, index, message }`,
//! and the conditions after it are still checked. Whatever the function emitted before it
//! failed stays. A decision the function asks for still pauses the command.
//!
//! The end is announced by `match_ended`, carrying `{ condition, index, winners }`, and kept in
//! the variable `core:victory.result` with the same fields.

mod conditions;

use std::collections::BTreeMap;

use strata_kernel::{Frac, ModuleId, SlotId, Sym, Value};
use strata_model::{Event, GameState, Logic, MapDef, PlayerId, ScenarioDef, VarKey};
use strata_rules::{
    ConfigSchema, Ctx, Halt, LogicError, MatchResult, ModuleError, NativeEnv, Progress, Registrar,
    RuleModule, Victory, logic_name,
};

use crate::rounds::ROUND_ENDED;
use crate::toy::no_config;

use conditions::Fault;
pub use conditions::WIN_FIELD;

/// The custom event that announces the end of the match.
pub const MATCH_ENDED: &str = "match_ended";
/// The custom event that reports a condition skipped because it failed.
pub const CONDITION_FAILED: &str = "victory_condition_failed";
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
    /// After a command.
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
    fn check(&self, scenario: &ScenarioDef, map: &MapDef) -> Result<(), LogicError> {
        conditions::check(scenario, map)
    }

    fn settle(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, events: &[Event]) -> Result<(), Halt> {
        let ended = rounds_ended(events).into_iter().map(Moment::RoundEnd);
        for moment in std::iter::once(Moment::Command).chain(ended) {
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
                    condition: logic_name(logic, WIN_FIELD),
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
        let checked = match logic {
            Logic::Native(d) => match conditions::parse(&d.0) {
                Ok(condition) => conditions::native(ctx, env, &condition, index, moment),
                Err(message) => Err(Fault::Broken(message)),
            },
            Logic::Script(_) => conditions::script(ctx, env, logic, moment),
        };
        let name = logic_name(logic, WIN_FIELD);
        match checked {
            Ok(Some(verdict)) => return end(ctx, name, index, verdict),
            Ok(None) => {}
            Err(Fault::Broken(message)) => report(ctx, name, index, message)?,
            Err(Fault::Halt(halt)) => return Err(halt),
        }
    }
    Ok(())
}

/// Announces that condition `index` failed and was skipped.
fn report(ctx: &mut dyn Ctx, condition: Sym, index: usize, message: String) -> Result<(), Halt> {
    let payload = Value::Map(BTreeMap::from([
        (Sym::new("condition"), Value::Str(condition)),
        (Sym::new("index"), Value::Int(index as i64)),
        (Sym::new("message"), Value::Str(Sym::new(&message))),
    ]));
    ctx.emit(Event::Custom {
        id: Sym::new(CONDITION_FAILED),
        payload,
    })
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
