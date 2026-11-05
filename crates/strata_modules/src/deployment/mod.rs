//! `core:deployment`: start zones, placement order and the placement command (design §6.1,
//! §11.2).
//!
//! Before the first round, players put their reserve figures on the board with `place_unit`. Each
//! command places the next reserve figure of one unit, on an anchor space and rotation. The
//! footprint must stand on the board's surfaces, inside the start zone of the player's scenario
//! slot, and clear of every other figure. Seats deploy in their slots' `deploy_order`: a player
//! may place only once every seat before theirs has nothing left in reserve.

mod check;

use strata_kernel::{ModuleId, Rotation, SlotId, Space, Sym, Value, ValueError};
use strata_model::{Command, PlayerId, UnitId};
use strata_rules::{CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, Registrar, RuleModule};

use crate::toy::no_config;

pub use self::check::{deploying, placement_event, start_zone};

#[cfg(test)]
mod tests;

/// The custom command that places a figure.
pub const PLACE_UNIT: &str = "place_unit";

/// One `place_unit` command: the next reserve figure of `unit` goes on `at`, turned by
/// `rotation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceUnit {
    /// The unit whose figure is placed.
    pub unit: UnitId,
    /// The figure's anchor space.
    pub at: Space,
    /// The figure's rotation.
    pub rotation: Rotation,
}

impl PlaceUnit {
    /// The command, with the payload `{ unit, q, r, z, rotation }`.
    pub fn command(&self) -> Command {
        let int = |name: &str, n: i64| (Sym::new(name), Value::Int(n));
        let payload = [
            int("unit", i64::from(self.unit.0)),
            int("q", i64::from(self.at.q)),
            int("r", i64::from(self.at.r)),
            int("z", i64::from(self.at.z)),
            int("rotation", i64::from(self.rotation.steps())),
        ];
        Command::Custom {
            id: Sym::new(PLACE_UNIT),
            payload: Value::Map(payload.into()),
        }
    }

    /// Reads the payload of a `place_unit` command.
    pub fn parse(payload: &Value) -> Result<Self, String> {
        let int = |name: &str| {
            let found = payload.get(name).and_then(Value::as_int);
            found.map_err(|e: ValueError| format!("{PLACE_UNIT} {name}: {e}"))
        };
        let range = |name: &str| format!("{PLACE_UNIT} {name}: out of range");
        let unit = u32::try_from(int("unit")?).map_err(|_| range("unit"))?;
        let q = i32::try_from(int("q")?).map_err(|_| range("q"))?;
        let r = i32::try_from(int("r")?).map_err(|_| range("r"))?;
        let z = u32::try_from(int("z")?).map_err(|_| range("z"))?;
        let steps = u8::try_from(int("rotation")?).ok();
        let rotation = steps
            .and_then(Rotation::new)
            .ok_or_else(|| range("rotation"))?;
        Ok(Self {
            unit: UnitId(unit),
            at: Space::new(q, r, z),
            rotation,
        })
    }
}

/// Handles `place_unit` by checking the placement and moving the figure onto the board.
struct Place;

impl CommandHandler for Place {
    fn kind(&self) -> &str {
        PLACE_UNIT
    }

    fn resolve(&self, ctx: &mut dyn Ctx, player: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("`place_unit` is a custom command"));
        };
        let place = PlaceUnit::parse(payload).map_err(Halt::reject)?;
        let event = placement_event(ctx.state(), ctx.env(), player, &place);
        ctx.emit(event.map_err(Halt::reject)?)
    }
}

/// The deployment slot.
pub struct Deployment;

impl RuleModule for Deployment {
    fn id(&self) -> ModuleId {
        ModuleId::new("core:deployment")
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
        Ok(())
    }
}
