//! What a placement must satisfy: the deployment order, the start zone, the board's surfaces and
//! the other figures.

use std::collections::BTreeSet;

use strata_kernel::{Hex, Space, Sym};
use strata_model::board::CompiledBoard;
use strata_model::{Event, FigureId, FigureStatus, GameState, PlayerId, ScenarioSlot};
use strata_rules::NativeEnv;

use super::PlaceUnit;

/// The scenario slot `player` fills.
fn slot<'a>(
    state: &GameState,
    env: &'a dyn NativeEnv,
    player: PlayerId,
) -> Option<&'a ScenarioSlot> {
    let seat = state.setup.players.iter().find(|p| p.id == player)?;
    env.scenario()?.slots.iter().find(|s| s.id == seat.slot)
}

/// Whether any unit `player` owns still has a figure in reserve.
fn has_reserve(state: &GameState, player: PlayerId) -> bool {
    let units = state.units.iter().filter(|u| u.owner == player);
    let mut figures = units.flat_map(|u| &u.figures);
    figures.any(|f| state.figures[f.index()].status == FigureStatus::Reserve)
}

/// The player who deploys now: of the seats with a figure in reserve, the one whose slot comes
/// first in `deploy_order`, the lower player on a tie. `None` once everyone has deployed.
pub fn deploying(state: &GameState, env: &dyn NativeEnv) -> Option<PlayerId> {
    let seats = state.setup.players.iter();
    let seats = seats.filter_map(|p| Some((slot(state, env, p.id)?.deploy_order, p.id)));
    let waiting = seats.filter(|(_, player)| has_reserve(state, *player));
    waiting.min().map(|(_, player)| player)
}

/// The name and columns of the zone `player`'s slot starts in.
pub fn start_zone<'a>(
    state: &GameState,
    env: &'a dyn NativeEnv,
    player: PlayerId,
) -> Result<(&'a Sym, &'a [Hex]), String> {
    let slot = slot(state, env, player);
    let slot = slot.ok_or_else(|| format!("player {} has no scenario slot", player.0))?;
    let cells = env.zone(slot.start.as_str());
    let cells = cells.ok_or_else(|| format!("the map has no zone `{}`", slot.start))?;
    Ok((&slot.start, cells))
}

/// Every space a figure on the board covers. A figure whose footprint cannot be worked out
/// covers its anchor.
fn taken(state: &GameState, env: &dyn NativeEnv, board: &CompiledBoard) -> BTreeSet<Space> {
    let on_board = state
        .figures
        .iter()
        .filter(|f| f.status == FigureStatus::OnBoard);
    let covered = on_board.filter_map(|f| {
        let at = f.location?;
        let footprint = env.footprint(&f.def);
        let spaces = footprint.and_then(|fp| fp.spaces(board, at, f.rotation).ok());
        Some(spaces.unwrap_or_else(|| vec![at]))
    });
    covered.flatten().collect()
}

/// The event that carries out `place` for `player`, or why the placement is illegal.
pub fn placement_event(
    state: &GameState,
    env: &dyn NativeEnv,
    player: PlayerId,
    place: &PlaceUnit,
) -> Result<Event, String> {
    if state.flow.round > 0 {
        return Err("deployment is over: the first round has begun".into());
    }
    let id = place.unit.0;
    let unit = state.units.get(place.unit.index());
    let unit = unit.ok_or_else(|| format!("no unit {id}"))?;
    if unit.owner != player {
        return Err(format!("unit {id} is not yours"));
    }
    let in_reserve = |f: &&FigureId| state.figures[f.index()].status == FigureStatus::Reserve;
    let figure = unit.figures.iter().find(in_reserve).copied();
    let figure = figure.ok_or_else(|| format!("unit {id} has no figure in reserve"))?;
    if let Some(first) = deploying(state, env).filter(|p| *p != player) {
        return Err(format!("player {} deploys first", first.0));
    }
    let (zone, cells) = start_zone(state, env, player)?;
    let board = env.board().ok_or("the match has no board")?;
    let def = &state.figures[figure.index()].def;
    let footprint = env.footprint(def);
    let footprint = footprint.ok_or_else(|| format!("figure {} has no footprint", figure.0))?;
    let spaces = footprint.spaces(board, place.at, place.rotation);
    let spaces = spaces.map_err(|e| e.to_string())?;
    if let Some(out) = spaces.iter().find(|s| !cells.contains(&s.hex())) {
        return Err(format!("{out:?} is outside start zone `{zone}`"));
    }
    let taken = taken(state, env, board);
    if let Some(space) = spaces.iter().find(|s| taken.contains(s)) {
        return Err(format!("{space:?} is taken by another figure"));
    }
    Ok(Event::FigureMoved {
        figure,
        to: place.at,
        rotation: place.rotation,
    })
}
