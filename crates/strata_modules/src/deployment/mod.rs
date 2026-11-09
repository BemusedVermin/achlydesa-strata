//! `core:deployment`: start zones, placement order and the setup step (design §6.1, §11.2).
//!
//! Before the first round, players take turns placing one unit each with the `place_unit`
//! command, in the order of their scenario slots' `deploy_order`, round and round. A player with
//! nothing left to place is skipped. A placement puts every figure of one of the player's
//! reserve units on a standable, empty surface inside the player's start zone; a figure with a
//! larger footprint, turned the way the payload asks, needs every cell of it there. Each cell
//! stands where the movement module would stand it, headroom included, so a figure is never
//! placed where it could not move (see `NativeEnv::cells`). When the last unit is down the
//! module emits `deployment_done`.
//!
//! The payload is `{ unit = <unit id>, at = { { q, r, z, o }, ... } }`: one anchor space and
//! rotation per figure, in the order the unit lists its figures, with `o` optional and `0` by
//! default. It is the slot's command contract, which any replacement module accepts; see
//! [`payload`]. [`check_unit`] is the validator, shared with anything that lists legal
//! placements, and [`deploying`] says whose turn it is.

pub mod payload;
pub mod placement;

use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{
    Command, Descriptor, Event, FigureStatus, GameState, Logic, PlayerId, TriggerDef, VarKey,
};
use strata_rules::{
    CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, NativeEnv, NativeLogic, Registrar,
    RuleModule,
};

use crate::toy::no_config;

pub use payload::{PLACE_UNIT, PlaceUnit, Spot};
pub use placement::{Misplaced, check_placement, check_unit};

/// The custom event emitted once every unit has been placed.
pub const DEPLOYMENT_DONE: &str = "deployment_done";
/// The event the command emits for the module's native logic to act on.
const REQUESTED: &str = "placement_requested";
/// The name the driver is registered under.
const DRIVE: &str = "core:deployment.drive";
/// The module that owns the variables.
const OWNER: &str = "core:deployment";

/// The deployment slot.
pub struct Deployment;

impl RuleModule for Deployment {
    fn id(&self) -> ModuleId {
        ModuleId::new(OWNER)
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("deployment"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        no_config()
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.command(Place);
        r.native(DRIVE, Box::new(Drive));
        let call = [(Sym::new("native"), Value::Str(Sym::new(DRIVE)))];
        r.trigger(TriggerDef {
            event: Sym::new(REQUESTED),
            filter: None,
            limit: None,
            effect: Logic::Native(Descriptor(Value::Map(call.into()))),
        });
        Ok(())
    }
}

/// Handles `place_unit`, which asks the driver to validate and carry out a placement.
struct Place;

impl CommandHandler for Place {
    fn kind(&self) -> &str {
        PLACE_UNIT
    }

    fn resolve(&self, ctx: &mut dyn Ctx, player: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("`place_unit` is a custom command"));
        };
        let mut asked = payload.clone();
        let who = Value::Int(i64::from(player.0));
        if let Value::Map(map) = &mut asked {
            map.insert(Sym::new("player"), who);
        }
        ctx.emit(Event::Custom {
            id: Sym::new(REQUESTED),
            payload: asked,
        })
    }
}

fn cursor_key() -> VarKey {
    VarKey {
        module: ModuleId::new(OWNER),
        name: Sym::new("cursor"),
    }
}

/// Whether `player` still has a unit waiting off the board.
fn waiting(state: &GameState, player: PlayerId) -> bool {
    let mine = state.units.iter().filter(|u| u.owner == player);
    let mut figures = mine.flat_map(|u| u.figures.iter());
    figures.any(|f| state.figures[f.index()].status == FigureStatus::Reserve)
}

/// Every player, in the order they place.
fn seating(state: &GameState, env: &dyn NativeEnv) -> Result<Vec<PlayerId>, Halt> {
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

/// The player whose turn it is to place a unit with `place_unit`.
///
/// `None` once the game has started, when nobody has a unit left in reserve, or when some
/// player has no start zone. The driver enforces this order, so the UI, the AI and tests can ask
/// it instead of working it out themselves.
pub fn deploying(state: &GameState, env: &dyn NativeEnv) -> Option<PlayerId> {
    if state.flow.round > 0 {
        return None;
    }
    let seats = seating(state, env).ok()?;
    let start = state.vars.get(&cursor_key()).and_then(|v| v.as_int().ok());
    let start = start.unwrap_or(0) as usize;
    let mut order = (0..seats.len()).map(|i| seats[(start + i) % seats.len()]);
    order.find(|player| waiting(state, *player))
}

/// Reads a `place_unit` payload with the `player` the command handler added.
fn parse(payload: &Value) -> Result<(PlayerId, PlaceUnit), Halt> {
    let player = payload.get("player").and_then(Value::as_int);
    let player = player.map_err(|e| Halt::reject(format!("place_unit: {e}")))?;
    let put = PlaceUnit::from_value(payload).map_err(Halt::reject)?;
    Ok((PlayerId(player as u8), put))
}

/// Checks a placement against the turn order, the unit and the board, then carries it out.
struct Drive;

impl NativeLogic for Drive {
    fn run(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, event: &Event) -> Result<(), Halt> {
        let Event::Custom { payload, .. } = event else {
            return Ok(());
        };
        let (player, put) = parse(payload)?;
        let state = ctx.state();
        if state.flow.round > 0 {
            return Err(Halt::reject("deployment is over: the game has started"));
        }
        let seats = seating(state, env)?;
        if deploying(state, env) != Some(player) {
            return Err(Halt::reject("it is not your turn to deploy"));
        }
        let bodies = check_unit(state, env, player, &put);
        let bodies = bodies.map_err(|e| Halt::reject(e.to_string()))?;
        for ((figure, cells), spot) in bodies.into_iter().zip(&put.at) {
            ctx.emit(Event::FigureMoved {
                figure,
                to: spot.anchor,
                rotation: spot.rotation,
                path: Vec::new(),
                cells,
            })?;
        }
        let seat = seats.iter().position(|p| *p == player).unwrap_or(0);
        let next = (seat + 1) % seats.len();
        let value = Value::Int(next as i64);
        ctx.emit(Event::VarSet {
            key: cursor_key(),
            value,
        })?;
        if seats.iter().any(|p| waiting(ctx.state(), *p)) {
            return Ok(());
        }
        let id = Sym::new(DEPLOYMENT_DONE);
        ctx.emit(Event::Custom {
            id,
            payload: Value::Nil,
        })
    }
}

#[cfg(test)]
mod tests;
