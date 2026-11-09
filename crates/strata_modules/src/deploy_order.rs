//! Whose turn it is to place a unit during setup (design §6.1, §11.2).
//!
//! Seats deploy in the order of their scenario slots' `deploy_order`, ties going to the lower
//! player number, and a seat with nothing left in reserve is skipped. `core:deployment`'s
//! `order` setting picks how the seats take turns: [`Order::Alternate`] or [`Order::Army`].

use strata_kernel::{ModuleId, Sym, Value};
use strata_model::{Event, FigureStatus, GameState, PlayerId, VarKey};
use strata_rules::{Ctx, Halt, NativeEnv, NextActor};

use crate::deployment::OWNER;

/// How the seats take turns placing their units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// One unit each, seat after seat, round and round (`order = "alternate"`, the default).
    Alternate,
    /// Each seat places its whole reserve before the next seat starts (`order = "army"`).
    Army,
}

impl Order {
    /// The order a config value names, `alternate` or `army`.
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "alternate" => Some(Self::Alternate),
            "army" => Some(Self::Army),
            _ => None,
        }
    }
}

/// The player whose turn it is to place a unit, or `None` once the game has started, when nobody
/// has a unit left in reserve, or when some player has no start zone.
pub fn deploying(state: &GameState, env: &dyn NativeEnv, order: Order) -> Option<PlayerId> {
    if state.flow.round > 0 {
        return None;
    }
    let seats = seating(state, env).ok()?;
    let start = match order {
        Order::Alternate => cursor(state) % seats.len().max(1),
        Order::Army => 0,
    };
    let mut turns = (0..seats.len()).map(|i| seats[(start + i) % seats.len()]);
    turns.find(|player| waiting(state, *player))
}

/// Whether any seat with a start zone still has a unit in reserve, whatever the round.
pub(crate) fn anyone_waiting(state: &GameState, env: &dyn NativeEnv) -> bool {
    let seats = seating(state, env).unwrap_or_default();
    seats.into_iter().any(|player| waiting(state, player))
}

/// Records that `player` has just placed, so in alternate order the next seat goes next.
pub(crate) fn placed(ctx: &mut dyn Ctx, seats: &[PlayerId], player: PlayerId) -> Result<(), Halt> {
    let seat = seats.iter().position(|p| *p == player).unwrap_or(0);
    let next = (seat + 1) % seats.len().max(1);
    ctx.emit(Event::VarSet {
        key: cursor_key(),
        value: Value::Int(next as i64),
    })
}

/// Every player, in the order they place.
pub(crate) fn seating(state: &GameState, env: &dyn NativeEnv) -> Result<Vec<PlayerId>, Halt> {
    let mut seats = Vec::new();
    for seat in 0..state.players.len() {
        let player = PlayerId(seat as u8);
        let slot = env.deployment(state, player);
        let slot = slot.ok_or_else(|| Halt::reject(format!("player {seat} has no start zone")))?;
        seats.push((slot.order, player));
    }
    seats.sort();
    Ok(seats.into_iter().map(|(_, player)| player).collect())
}

/// Whether `player` still has a unit waiting off the board.
pub(crate) fn waiting(state: &GameState, player: PlayerId) -> bool {
    let mine = state.units.iter().filter(|u| u.owner == player);
    let mut figures = mine.flat_map(|u| u.figures.iter());
    figures.any(|f| state.figures[f.index()].status == FigureStatus::Reserve)
}

/// The seat, by position in the placing order, whose turn comes next in alternate order.
fn cursor(state: &GameState) -> usize {
    let at = state.vars.get(&cursor_key()).and_then(|v| v.as_int().ok());
    at.unwrap_or(0) as usize
}

fn cursor_key() -> VarKey {
    VarKey {
        module: ModuleId::new(OWNER),
        name: Sym::new("cursor"),
    }
}

/// Names the deploying player as the one to act while units wait to be placed, so the session
/// asks them, not the active player, for the next command.
pub(crate) struct DeployTurn(pub(crate) Order);

impl NextActor for DeployTurn {
    fn next_actor(&self, state: &GameState, env: &dyn NativeEnv) -> Option<PlayerId> {
        deploying(state, env, self.0)
    }
}

#[cfg(test)]
mod tests;
