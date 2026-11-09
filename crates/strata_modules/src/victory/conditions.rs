//! The native victory conditions, their tiebreaks and their progress estimates.
//!
//! Figures count for teams in two ways. Holding a zone is about who stands there, so a figure
//! counts for the team of the player who controls it now. Elimination and points are about whose
//! army it is, so a figure counts for the team of its unit's owner, even while an enemy controls
//! it.

use std::collections::{BTreeMap, BTreeSet};

use strata_kernel::{Frac, Hex, Sym, Value};
use strata_model::{Event, FigureStatus, GameState, Logic, MapDef, ScenarioDef, UnitState};
use strata_rules::{Ctx, Halt, LogicError, NativeEnv};

use super::{Moment, Verdict, key, share};
use crate::shared::names;

/// The descriptor field that names a native condition.
pub const WIN_FIELD: &str = "win";

/// A native victory condition, read from its descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Condition {
    /// Every team but one has no figure left in play.
    EliminateAll,
    /// One team alone stands in `zone` at `rounds` round ends in a row.
    HoldZone {
        /// The map zone.
        zone: Sym,
        /// How many round ends in a row.
        rounds: i64,
    },
    /// The end of round `rounds` ends the match, settled by the tiebreaks in order.
    RoundLimit {
        /// The last round.
        rounds: i64,
        /// The tiebreaks, in order.
        tiebreaks: Vec<Tiebreak>,
    },
}

/// A way to rank the teams when the round limit ends the match; higher is better.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tiebreak {
    /// Points of the team's units with a figure in play.
    PointsRemaining,
    /// Points of the other teams' units with no figure in play.
    PointsDestroyed,
    /// The team's figures in play.
    FiguresRemaining,
}

impl Tiebreak {
    fn named(name: &str) -> Option<Self> {
        match name {
            "points_remaining" => Some(Tiebreak::PointsRemaining),
            "points_destroyed" => Some(Tiebreak::PointsDestroyed),
            "figures_remaining" => Some(Tiebreak::FiguresRemaining),
            _ => None,
        }
    }
}

/// Why a condition could not be checked.
pub(crate) enum Fault {
    /// The condition itself is broken, such as a Lua function that fails: it is skipped.
    Broken(String),
    /// The command must stop, as for a decision a script asks for.
    Halt(Halt),
}

impl From<Halt> for Fault {
    fn from(halt: Halt) -> Self {
        Fault::Halt(halt)
    }
}

/// Reads the native condition `win`, or says what is wrong with it.
pub(crate) fn parse(win: &Value) -> Result<Condition, String> {
    let kind = win.get(WIN_FIELD).and_then(Value::as_str);
    let kind = kind.map_err(|e| format!("a victory condition needs `{WIN_FIELD}`: {e}"))?;
    let bad = |e: String| format!("win.{kind}: {e}");
    match kind {
        "eliminate_all" => Ok(Condition::EliminateAll),
        "hold_zone" => {
            let zone = win.get("zone").and_then(Value::as_str);
            let zone = zone.map_err(|e| bad(e.to_string()))?;
            Ok(Condition::HoldZone {
                zone: Sym::new(zone),
                rounds: rounds(win, Some(1)).map_err(bad)?,
            })
        }
        "round_limit" => {
            let rounds = rounds(win, None).map_err(bad)?;
            let listed = names(win, "tiebreak").map_err(|e| bad(e.to_string()))?;
            let tiebreak = |name: &Sym| {
                let found = Tiebreak::named(name.as_str());
                found.ok_or_else(|| bad(format!("unknown tiebreak `{name}`")))
            };
            let tiebreaks = listed.iter().map(tiebreak).collect::<Result<_, _>>()?;
            Ok(Condition::RoundLimit { rounds, tiebreaks })
        }
        other => Err(format!("core:victory has no condition `{other}`")),
    }
}

/// The `rounds` field, at least one, or `default` when it is missing.
fn rounds(win: &Value, default: Option<i64>) -> Result<i64, String> {
    let given = match (win.get("rounds"), default) {
        (Ok(Value::Nil) | Err(_), Some(d)) => d,
        (Ok(v), _) => v.as_int().map_err(|e| e.to_string())?,
        (Err(e), None) => return Err(e.to_string()),
    };
    (given >= 1)
        .then_some(given)
        .ok_or_else(|| "rounds must be at least 1".to_owned())
}

/// Checks every native condition of `scenario` before a match, with `map` the scenario's map.
pub(crate) fn check(scenario: &ScenarioDef, map: &MapDef) -> Result<(), LogicError> {
    for (index, logic) in scenario.victory.iter().enumerate() {
        let Logic::Native(descriptor) = logic else {
            continue;
        };
        let at = |e: String| LogicError(format!("victory condition {}: {e}", index + 1));
        let condition = parse(&descriptor.0).map_err(at)?;
        if let Condition::HoldZone { zone, .. } = &condition {
            if !map.zones.contains_key(zone) {
                return Err(at(format!("win.hold_zone: the map has no zone `{zone}`")));
            }
        }
    }
    Ok(())
}

/// Checks the native condition `condition` at `moment`.
pub(crate) fn native(
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    condition: &Condition,
    index: usize,
    moment: Moment,
) -> Result<Option<Verdict>, Fault> {
    match (condition, moment) {
        (Condition::EliminateAll, _) => Ok(eliminate_all(ctx.state())),
        (Condition::HoldZone { zone, rounds }, Moment::RoundEnd(_)) => {
            hold_zone(ctx, env, zone, *rounds, index)
        }
        (Condition::RoundLimit { rounds, tiebreaks }, Moment::RoundEnd(round)) => {
            Ok((i64::from(round) >= *rounds).then(|| round_limit(ctx.state(), env, tiebreaks)))
        }
        (Condition::HoldZone { .. } | Condition::RoundLimit { .. }, Moment::Command) => Ok(None),
    }
}

/// Calls a Lua condition with the moment and reads its verdict.
pub(crate) fn script(
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    logic: &Logic,
    moment: Moment,
) -> Result<Option<Verdict>, Fault> {
    let when = match moment {
        Moment::Command => "command",
        Moment::RoundEnd(_) => "round_end",
    };
    let found = match env.call_logic(ctx, logic, &[Value::Str(Sym::new(when))]) {
        Ok(found) => found,
        Err(Halt::Rejected(violation)) => return Err(Fault::Broken(violation.0)),
        Err(halt) => return Err(Fault::Halt(halt)),
    };
    match found {
        Value::Nil | Value::Bool(false) => Ok(None),
        Value::Int(team) => match u8::try_from(team) {
            Ok(team) => Ok(Some(Verdict::Team(team))),
            Err(_) => Err(Fault::Broken(format!(
                "returned team {team}, which is no team"
            ))),
        },
        Value::Str(s) if s.as_str() == "draw" => Ok(Some(Verdict::Draw)),
        other => Err(Fault::Broken(format!("returned {}", other.type_name()))),
    }
}

/// Every team that fielded a figure, by unit owner, and whether it still has one in play.
fn armies(state: &GameState) -> BTreeMap<u8, bool> {
    let mut teams = BTreeMap::new();
    for unit in state.units.iter().filter(|u| !u.figures.is_empty()) {
        *teams.entry(state.team(unit.owner)).or_insert(false) |= state.unit_in_play(unit);
    }
    teams
}

fn eliminate_all(state: &GameState) -> Option<Verdict> {
    let teams = armies(state);
    let mut standing = teams.iter().filter(|(_, alive)| **alive).map(|(t, _)| *t);
    match (teams.len(), standing.next(), standing.next()) {
        (0 | 1, ..) | (_, Some(_), Some(_)) => None,
        (_, Some(team), None) => Some(Verdict::Team(team)),
        (_, None, _) => Some(Verdict::Draw),
    }
}

/// The variable counting how many round ends in a row a team has held condition `index`'s
/// zone: `{ team, rounds }`.
fn held_key(index: usize) -> strata_model::VarKey {
    key(&format!("held.{index}"))
}

/// The team holding condition `index`'s zone and for how many round ends in a row.
fn held(state: &GameState, index: usize) -> Option<(u8, i64)> {
    let found = state.vars.get(&held_key(index))?;
    let team = found.get("team").ok()?.as_int().ok()?;
    Some((team as u8, found.get("rounds").ok()?.as_int().ok()?))
}

/// The teams, by controller, whose figures on the board cover some space of `zone`.
fn holders(state: &GameState, zone: &[Hex]) -> BTreeSet<u8> {
    let covered = state.board_changes.occupancy.iter();
    let inside = covered.filter(|(space, _)| zone.contains(&space.hex()));
    let standing = inside.filter_map(|(_, f)| state.figures.get(f.index()));
    let standing = standing.filter(|f| f.status == FigureStatus::OnBoard);
    standing.map(|f| state.team(f.controller)).collect()
}

fn hold_zone(
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    zone: &Sym,
    needed: i64,
    index: usize,
) -> Result<Option<Verdict>, Fault> {
    let state = ctx.state();
    let hexes = env.zone(state, zone.as_str());
    let hexes = hexes.ok_or_else(|| Fault::Broken(format!("the map has no zone `{zone}`")))?;
    let mut teams = holders(state, &hexes).into_iter();
    let holder = match (teams.next(), teams.next()) {
        (Some(team), None) => Some(team),
        _ => None,
    };
    let before = held(state, index);
    let count = match (holder, before) {
        (Some(team), Some((was, n))) if team == was => n + 1,
        (Some(_), _) => 1,
        (None, _) => 0,
    };
    let value = match holder {
        Some(team) => Value::Map(BTreeMap::from([
            (Sym::new("team"), Value::Int(i64::from(team))),
            (Sym::new("rounds"), Value::Int(count)),
        ])),
        None => Value::Nil,
    };
    if before.is_some() || holder.is_some() {
        let key = held_key(index);
        ctx.emit(Event::VarSet { key, value })?;
    }
    Ok(holder.filter(|_| count >= needed).map(Verdict::Team))
}

/// The verdict at the round limit: each tiebreak keeps the teams that score highest on it.
fn round_limit(state: &GameState, env: &dyn NativeEnv, tiebreaks: &[Tiebreak]) -> Verdict {
    let mut teams: Vec<u8> = state.players.iter().map(|p| p.team).collect();
    teams.sort_unstable();
    teams.dedup();
    for tiebreak in tiebreaks {
        if teams.len() <= 1 {
            break;
        }
        let scored: Vec<(i64, u8)> = teams
            .iter()
            .map(|team| (score(state, env, *tiebreak, *team), *team))
            .collect();
        let best = scored.iter().map(|(s, _)| *s).max();
        teams = scored
            .into_iter()
            .filter(|(s, _)| Some(*s) == best)
            .map(|(_, t)| t)
            .collect();
    }
    match teams.as_slice() {
        [team] => Verdict::Team(*team),
        _ => Verdict::Draw,
    }
}

/// The printed `points` stat of `unit`'s definition, the same price `core:army` reads; 0 when
/// the unit prints none or the packs define no `points` stat.
fn points(env: &dyn NativeEnv, unit: &UnitState) -> i64 {
    env.points(unit.def).unwrap_or(0)
}

/// `team`'s score on `tiebreak`, with units counted for their owner's team; higher is better.
fn score(state: &GameState, env: &dyn NativeEnv, tiebreak: Tiebreak, team: u8) -> i64 {
    let units = state.units.iter().filter(|u| !u.figures.is_empty());
    let ours = |u: &&UnitState| state.team(u.owner) == team;
    match tiebreak {
        Tiebreak::PointsRemaining => units
            .filter(ours)
            .filter(|u| state.unit_in_play(u))
            .map(|u| points(env, u))
            .sum(),
        Tiebreak::PointsDestroyed => units
            .filter(|u| !ours(u) && !state.unit_in_play(u))
            .map(|u| points(env, u))
            .sum(),
        Tiebreak::FiguresRemaining => {
            let figures = units.filter(ours).flat_map(|u| u.figures.iter());
            figures.filter(|f| state.in_play(**f)).count() as i64
        }
    }
}

/// How close `team` is to meeting the native condition `win`, if it has an estimate.
pub(crate) fn progress(
    state: &GameState,
    win: &Value,
    index: usize,
    team: u8,
) -> Result<Option<Frac>, LogicError> {
    match parse(win).map_err(LogicError)? {
        Condition::EliminateAll => {
            let theirs = state.units.iter().filter(|u| state.team(u.owner) != team);
            let figures: Vec<_> = theirs.flat_map(|u| u.figures.iter()).collect();
            let gone = figures.iter().filter(|f| !state.in_play(***f)).count();
            Ok(share(gone, figures.len()))
        }
        Condition::HoldZone { rounds, .. } => {
            let count = match held(state, index) {
                Some((holder, n)) if holder == team => n,
                _ => 0,
            };
            Ok(share(count.max(0) as usize, rounds as usize))
        }
        Condition::RoundLimit { .. } => Ok(None),
    }
}
