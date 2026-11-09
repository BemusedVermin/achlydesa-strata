//! `toy:driver`: turns the `attack` command into an `attack_declared` event.

use strata_kernel::{ModuleId, Rotation, SlotId, Space, Sym, Value};
use strata_model::{Command, Event, FigureId, PlayerId};
use strata_rules::{CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, Registrar, RuleModule};

use super::{ATTACK_DECLARED, no_config};

/// Handles `attack`: its payload becomes the payload of the declared attack.
struct Attack;

impl CommandHandler for Attack {
    fn kind(&self) -> &str {
        "attack"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("`attack` is a custom command"));
        };
        ctx.emit(Event::Custom {
            id: Sym::new(ATTACK_DECLARED),
            payload: payload.clone(),
        })
    }
}

/// Handles `move_to`, a stand-in for the movement action in tests: the payload `{ figure, q, r, z }`
/// puts the figure there, whatever the terrain.
struct MoveTo;

impl CommandHandler for MoveTo {
    fn kind(&self) -> &str {
        "move_to"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("`move_to` is a custom command"));
        };
        let int = |name: &str| {
            let found = payload.get(name).and_then(Value::as_int);
            found.map_err(|e| Halt::reject(format!("move_to {name}: {e}")))
        };
        let figure = FigureId(int("figure")? as u32);
        let to = Space::new(int("q")? as i32, int("r")? as i32, int("z")? as u32);
        let rotation = Rotation::default();
        ctx.emit(Event::FigureMoved {
            figure,
            to,
            rotation,
            path: Vec::new(),
        })
    }
}

/// The add-on every toy ruleset shares, whichever combat module it uses.
pub struct Driver;

impl RuleModule for Driver {
    fn id(&self) -> ModuleId {
        ModuleId::new("toy:driver")
    }

    fn slot(&self) -> Option<SlotId> {
        None
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        no_config()
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.command(Attack);
        r.command(MoveTo);
        Ok(())
    }
}
