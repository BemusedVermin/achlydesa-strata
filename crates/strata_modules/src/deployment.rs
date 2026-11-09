//! `core:deployment`: start zones, placement order and the setup step (design §6.1, §6.4, §11.2).
//!
//! Before the first round, players place their units with the `place_unit` command, in the
//! order of their scenario slots' `deploy_order`: one unit each, round and round, or with
//! `order = "army"` each seat's whole reserve in turn (see [`crate::deploy_order`]). A player
//! with nothing left to place is skipped. A placement puts every figure of one of the player's
//! reserve units on a standable, empty surface inside the player's start zone; a figure with a
//! larger footprint needs every cell of it there. Each cell stands where the movement module
//! would stand it, headroom included, so a figure is never placed where it could not move (see
//! `NativeEnv::cells`). When the last unit is down the module emits `deployment_done`.
//!
//! Placing happens in the `deploy` step of the `setup` phase, which the module registers (see
//! [`crate::setup`]). The first placement opens it, and the last one closes `setup`, which
//! starts round 1; `core:rounds` refuses to start it earlier. While units
//! wait, the module names the deploying player as the one who acts next, so the session asks
//! them for the next command.
//!
//! The payload is `{ unit = <unit id>, at = { { q, r, z }, ... } }`: one space per figure, in
//! the order the unit lists its figures. Figures face the default way.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use strata_kernel::{ModuleId, Rotation, SlotId, Space, Sym, Value};
use strata_model::{
    Command, ConfigField, Descriptor, Event, FigureId, FigureStatus, GameState, Logic, PlayerId,
    TriggerDef, UnitId,
};
use strata_rules::{
    CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, NativeEnv, NativeLogic, NextActor,
    Registrar, RuleModule,
};

use crate::deploy_order::{DeployTurn, Order, deploying, placed, seating, waiting};
use crate::setup::{self, DEPLOY};

/// The custom event emitted once every unit has been placed.
pub const DEPLOYMENT_DONE: &str = "deployment_done";
/// The event the command emits for the module's native logic to act on.
const REQUESTED: &str = "placement_requested";
/// The name the driver is registered under.
const DRIVE: &str = "core:deployment.drive";
/// The module's ID.
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
        static SCHEMA: OnceLock<ConfigSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            let order = ConfigField {
                kind: Sym::new("str"),
                default: Value::Str(Sym::new("alternate")),
            };
            ConfigSchema::new(BTreeMap::from([(Sym::new("order"), order)]))
        })
    }

    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError> {
        let named = cfg.get("order").and_then(Value::as_str);
        let named = named.map_err(|e| ModuleError::Invalid(e.to_string()))?;
        let order = Order::named(named).ok_or_else(|| {
            ModuleError::Invalid(format!(
                "order must be `alternate` or `army`, not `{named}`"
            ))
        })?;
        setup::register(r);
        r.command(Place);
        r.service::<dyn NextActor>(Box::new(DeployTurn(order)));
        r.native(DRIVE, Box::new(Drive { order }));
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
        "place_unit"
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

/// The reading of one `place_unit` payload.
struct Placement {
    player: PlayerId,
    unit: UnitId,
    at: Vec<Space>,
}

fn parse(payload: &Value) -> Result<Placement, Halt> {
    let bad = |e: strata_kernel::ValueError| Halt::reject(format!("place_unit: {e}"));
    let int = |v: &Value| v.as_int().map_err(bad);
    let player = int(payload.get("player").map_err(bad)?)?;
    let unit = int(payload.get("unit").map_err(bad)?)?;
    let mut at = Vec::new();
    for cell in payload.get("at").and_then(Value::as_list).map_err(bad)? {
        let cell = cell.as_list().map_err(bad)?;
        let [q, r, z] = cell else {
            return Err(Halt::reject("place_unit: each space is { q, r, z }"));
        };
        let z = u32::try_from(int(z)?).map_err(|_| Halt::reject("place_unit: z is negative"))?;
        at.push(Space::new(int(q)? as i32, int(r)? as i32, z));
    }
    Ok(Placement {
        player: PlayerId(player as u8),
        unit: UnitId(unit as u32),
        at,
    })
}

/// Checks a placement against the turn order, the unit and the board, then carries it out.
struct Drive {
    order: Order,
}

impl NativeLogic for Drive {
    fn run(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, event: &Event) -> Result<(), Halt> {
        match event {
            Event::Custom { payload, .. } => self.place(ctx, env, payload),
            _ => Ok(()),
        }
    }
}

impl Drive {
    fn place(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, payload: &Value) -> Result<(), Halt> {
        let placement = parse(payload)?;
        if ctx.state().flow.round > 0 {
            return Err(Halt::reject("deployment is over: the game has started"));
        }
        if let Some(tree) = env.phases() {
            setup::open(ctx, tree)?;
            if setup::current(ctx.state(), tree).map(Sym::as_str) != Some(DEPLOY) {
                return Err(Halt::reject("units are placed only in the deploy step"));
            }
        }
        let state = ctx.state();
        let seats = seating(state, env)?;
        if deploying(state, env, self.order) != Some(placement.player) {
            return Err(Halt::reject("it is not your turn to deploy"));
        }
        let figures = check_unit(state, &placement)?;
        let bodies = check_spaces(state, env, &placement, &figures)?;
        for ((figure, to), body) in figures.into_iter().zip(&placement.at).zip(bodies) {
            ctx.emit(Event::FigureMoved {
                figure,
                to: *to,
                rotation: Rotation::default(),
                path: Vec::new(),
                cells: body,
            })?;
        }
        placed(ctx, &seats, placement.player)?;
        if seats.iter().any(|p| waiting(ctx.state(), *p)) {
            return Ok(());
        }
        let id = Sym::new(DEPLOYMENT_DONE);
        ctx.emit(Event::Custom {
            id,
            payload: Value::Nil,
        })?;
        match env.phases() {
            Some(tree) => setup::finish_step(ctx, tree),
            None => Ok(()),
        }
    }
}

/// The figures of the placed unit, which must be the player's and waiting in reserve.
fn check_unit(state: &GameState, put: &Placement) -> Result<Vec<FigureId>, Halt> {
    let unit = state.units.get(put.unit.index());
    let unit = unit.filter(|u| u.owner == put.player);
    let unit = unit.ok_or_else(|| Halt::reject("that is not your unit"))?;
    let reserve = |f: &&FigureId| state.figures[f.index()].status == FigureStatus::Reserve;
    if unit.figures.iter().filter(reserve).count() != unit.figures.len() {
        return Err(Halt::reject("that unit is already on the board"));
    }
    if put.at.len() != unit.figures.len() {
        let (want, got) = (unit.figures.len(), put.at.len());
        return Err(Halt::reject(format!(
            "the unit has {want} figures, not {got}"
        )));
    }
    Ok(unit.figures.clone())
}

/// Every figure's body, with its anchor where the placement puts it, must stand on standable,
/// empty surfaces of the player's start zone, and no two bodies may share a space. Returns the
/// spaces each figure covers.
fn check_spaces(
    state: &GameState,
    env: &dyn NativeEnv,
    put: &Placement,
    figures: &[FigureId],
) -> Result<Vec<Vec<Space>>, Halt> {
    let slot = env.deployment(state, put.player);
    let slot = slot.ok_or_else(|| Halt::reject("you have no start zone"))?;
    let mut used = BTreeSet::new();
    let mut bodies = Vec::new();
    for (figure, anchor) in figures.iter().zip(&put.at) {
        let fits = |space: &Space| -> Result<(), Halt> {
            if !slot.zone.contains(&space.hex()) {
                return Err(Halt::reject("that space is outside your start zone"));
            }
            if !env.standable(*space) {
                return Err(Halt::reject("a figure cannot stand there"));
            }
            Ok(())
        };
        fits(anchor)?;
        let body = env.cells(state, *figure, *anchor, Rotation::default());
        let body = body.ok_or_else(|| Halt::reject("a figure cannot stand there"))?;
        for space in &body {
            fits(space)?;
            if state.occupant(*space).is_some() || !used.insert(*space) {
                return Err(Halt::reject("that space is occupied"));
            }
        }
        bodies.push(body);
    }
    Ok(bodies)
}
