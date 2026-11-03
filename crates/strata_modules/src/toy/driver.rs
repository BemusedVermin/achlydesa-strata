//! `toy:driver`: turns the `attack` command into an `attack_declared` event.

use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{Command, Event, PlayerId};
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
        Ok(())
    }
}
