//! The native victory conditions, their tiebreaks and their progress estimates.

use std::collections::{BTreeMap, BTreeSet};

use strata_kernel::{Frac, Hex, Sym, Value, ValueError};
use strata_model::{Descriptor, Event, FigureStatus, GameState, Logic, UnitState};
use strata_rules::{Ctx, Halt, LogicError, NativeEnv};

use super::{Moment, Verdict, key, share};

/// The descriptor field that names a native condition.
pub const WIN_FIELD: &str = "win";
/// The condition name reported for a Lua function.
pub const SCRIPT_CONDITION: &str = "script";
/// The stat that prices a unit.
const POINTS: &str = "points";

/// The name of a condition: the descriptor's `win` field, or `script`.
pub(crate) fn name(logic: &Logic) -> Sym {
    match logic {
        Logic::Native(Descriptor(value)) => {
            let name = value.get(WIN_FIELD).and_then(Value::as_str);
            Sym::new(name.unwrap_or("unnamed"))
        }
        Logic::Script(_) => Sym::new(SCRIPT_CONDITION),
    }
}

fn reject(kind: &str, e: impl std::fmt::Display) -> Halt {
    Halt::reject(format!("win.{kind}: {e}"))
}

/// Whether a figure is still in play.
fn live(state: &GameState, figure: &strata_model::FigureId) -> bool {
    let status = state.figures.get(figure.index()).map(|f| f.status);
    matches!(status, Some(FigureStatus::Reserve | FigureStatus::OnBoard))
}

/// Whether some figure of `unit` is still in play.
fn unit_live(state: &GameState, unit: &UnitState) -> bool {
    unit.figures.iter().any(|f| live(state, f))
}

/// Checks the native condition `win` at `moment`.
pub(crate) fn native(
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    win: &Value,
    index: usize,
    moment: Moment,
) -> Result<Option<Verdict>, Halt> {
    let kind = win.get(WIN_FIELD).and_then(Value::as_str);
    let kind = kind.map_err(|e| Halt::reject(format!("a victory condition needs `win`: {e}")))?;
    match (kind, moment) {
        ("eliminate_all", _) => Ok(eliminate_all(ctx.state())),
        ("hold_zone", Moment::RoundEnd(_)) => hold_zone(ctx, env, win, index),
        ("round_limit", Moment::RoundEnd(round)) => round_limit(ctx.state(), env, win, round),
        ("hold_zone" | "round_limit", Moment::Command) => Ok(None),
        (other, _) => Err(Halt::reject(format!(
            "core:victory has no condition `{other}`"
        ))),
    }
}

/// Calls a Lua condition with the moment and reads its verdict.
pub(crate) fn script(
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    logic: &Logic,
    moment: Moment,
) -> Result<Option<Verdict>, Halt> {
    let when = match moment {
        Moment::Command => "command",
        Moment::RoundEnd(_) => "round_end",
    };
    match env.call_logic(ctx, logic, &[Value::Str(Sym::new(when))])? {
        Value::Nil | Value::Bool(false) => Ok(None),
        Value::Int(team) => {
            let team = u8::try_from(team).map_err(|_| reject("script", "no such team"))?;
            Ok(Some(Verdict::Team(team)))
        }
        Value::Str(s) if s.as_str() == "draw" => Ok(Some(Verdict::Draw)),
        other => Err(reject("script", format!("returned {}", other.type_name()))),
    }
}

/// Every team that fielded a figure, and whether it still has one in play.
fn armies(state: &GameState) -> BTreeMap<u8, bool> {
    let mut teams = BTreeMap::new();
    for unit in state.units.iter().filter(|u| !u.figures.is_empty()) {
        *teams.entry(state.team(unit.owner)).or_insert(false) |= unit_live(state, unit);
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

/// The teams whose figures on the board cover some space of `zone`.
fn holders(state: &GameState, zone: &[Hex]) -> BTreeSet<u8> {
    let covered = state.board_changes.occupancy.iter();
    let inside = covered.filter(|(space, _)| zone.contains(&space.hex()));
    let standing = inside.filter_map(|(_, f)| state.figures.get(f.index()));
    let standing = standing.filter(|f| f.status == FigureStatus::OnBoard);
    standing.map(|f| state.team(f.controller)).collect()
}

/// The `rounds` field, at least one, or `default` when it is missing.
fn rounds(win: &Value, kind: &str, default: Option<i64>) -> Result<i64, Halt> {
    let given = match (win.get("rounds"), default) {
        (Ok(v), _) => v.as_int().map_err(|e| reject(kind, e))?,
        (Err(_), Some(d)) => d,
        (Err(e), None) => return Err(reject(kind, e)),
    };
    (given >= 1)
        .then_some(given)
        .ok_or_else(|| reject(kind, "rounds must be at least 1"))
}

fn hold_zone(
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    win: &Value,
    index: usize,
) -> Result<Option<Verdict>, Halt> {
    let zone = win.get("zone").and_then(Value::as_str);
    let zone = zone.map_err(|e| reject("hold_zone", e))?;
    let needed = rounds(win, "hold_zone", Some(1))?;
    let state = ctx.state();
    let hexes = env.zone(state, zone);
    let hexes = hexes.ok_or_else(|| reject("hold_zone", format!("no zone `{zone}`")))?;
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

fn round_limit(
    state: &GameState,
    env: &dyn NativeEnv,
    win: &Value,
    round: u32,
) -> Result<Option<Verdict>, Halt> {
    let limit = rounds(win, "round_limit", None)?;
    if i64::from(round) < limit {
        return Ok(None);
    }
    let breaks = match win.get("tiebreak") {
        Err(_) | Ok(Value::Nil) => Vec::new(),
        Ok(Value::Map(map)) if map.is_empty() => Vec::new(),
        Ok(Value::Str(one)) => vec![one.clone()],
        Ok(list) => {
            let items = list.as_list().map_err(|e| reject("round_limit", e))?;
            let names = items.iter().map(|v| v.as_str().map(Sym::new));
            let names: Result<Vec<Sym>, ValueError> = names.collect();
            names.map_err(|e| reject("round_limit", e))?
        }
    };
    let mut teams: Vec<u8> = state.players.iter().map(|p| p.team).collect();
    teams.sort_unstable();
    teams.dedup();
    for tiebreak in &breaks {
        if teams.len() <= 1 {
            break;
        }
        let mut scored = Vec::new();
        for team in &teams {
            scored.push((score(state, env, tiebreak.as_str(), *team)?, *team));
        }
        let best = scored.iter().map(|(s, _)| *s).max();
        teams = scored
            .into_iter()
            .filter(|(s, _)| Some(*s) == best)
            .map(|(_, t)| t)
            .collect();
    }
    Ok(Some(match teams.as_slice() {
        [team] => Verdict::Team(*team),
        _ => Verdict::Draw,
    }))
}

/// The printed points of `unit`, read through the `points` stat of its first figure.
fn points(state: &GameState, env: &dyn NativeEnv, unit: &UnitState) -> Result<i64, Halt> {
    let Some(figure) = unit.figures.first() else {
        return Ok(0);
    };
    let found = env.stat(state, *figure, POINTS);
    found.map_err(|e| reject("round_limit", e))
}

/// `team`'s score on `tiebreak`; higher is better.
fn score(state: &GameState, env: &dyn NativeEnv, tiebreak: &str, team: u8) -> Result<i64, Halt> {
    let units = state.units.iter().filter(|u| !u.figures.is_empty());
    let ours = |u: &&UnitState| state.team(u.owner) == team;
    match tiebreak {
        "points_remaining" => units
            .filter(ours)
            .filter(|u| unit_live(state, u))
            .map(|u| points(state, env, u))
            .sum(),
        "points_destroyed" => units
            .filter(|u| !ours(u) && !unit_live(state, u))
            .map(|u| points(state, env, u))
            .sum(),
        "figures_remaining" => {
            let figures = units.filter(ours).flat_map(|u| u.figures.iter());
            Ok(figures.filter(|f| live(state, f)).count() as i64)
        }
        other => Err(reject("round_limit", format!("unknown tiebreak `{other}`"))),
    }
}

/// How close `team` is to meeting the native condition `win`, if it has an estimate.
pub(crate) fn progress(
    state: &GameState,
    win: &Value,
    index: usize,
    team: u8,
) -> Result<Option<Frac>, LogicError> {
    let kind = win.get(WIN_FIELD).and_then(Value::as_str);
    match kind.map_err(|e| LogicError(e.to_string()))? {
        "eliminate_all" => {
            let theirs = state.units.iter().filter(|u| state.team(u.owner) != team);
            let figures: Vec<_> = theirs.flat_map(|u| u.figures.iter()).collect();
            let gone = figures.iter().filter(|f| !live(state, f)).count();
            Ok(share(gone, figures.len()))
        }
        "hold_zone" => {
            let needed = win
                .get("rounds")
                .and_then(Value::as_int)
                .unwrap_or(1)
                .max(1);
            let count = match held(state, index) {
                Some((holder, n)) if holder == team => n,
                _ => 0,
            };
            Ok(share(count.max(0) as usize, needed as usize))
        }
        _ => Ok(None),
    }
}
