//! Falling damage: the native rule `core:movement` hangs on `figure_fell` (design §6.1).
//!
//! With `fall_wounds = n` above zero, a figure that falls takes `n` wounds for every level it
//! dropped beyond `max_fall`, through `core:units`. With the default of zero the module deals no
//! damage, and a pack can write its own rule as a trigger on `figure_fell`.

use strata_kernel::{Sym, Value};
use strata_model::{Descriptor, Event, FigureId, FigureStatus, Logic, TriggerDef};
use strata_rules::{Ctx, Halt, NativeEnv, NativeLogic, Registrar};

use super::api::FIGURE_FELL;
use crate::units::wound;

/// The name the rule is registered under.
const NAME: &str = "core:movement.fall";

/// Wounds a fallen figure.
pub(super) struct Fall {
    /// Wounds per level dropped beyond `max_fall`.
    pub per_level: u32,
    /// The deepest drop that does not fall.
    pub max_fall: u32,
}

impl Fall {
    /// Registers the rule as a trigger on `figure_fell`, unless it deals no damage.
    pub fn register(self, r: &mut Registrar) {
        if self.per_level == 0 {
            return;
        }
        r.native(NAME, Box::new(self));
        let call = [(Sym::new("native"), Value::Str(Sym::new(NAME)))];
        r.trigger(TriggerDef {
            event: Sym::new(FIGURE_FELL),
            filter: None,
            limit: None,
            effect: Logic::Native(Descriptor(Value::Map(call.into()))),
        });
    }
}

impl NativeLogic for Fall {
    fn run(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, event: &Event) -> Result<(), Halt> {
        let Event::Custom { payload, .. } = event else {
            return Ok(());
        };
        let bad = |e: strata_kernel::ValueError| Halt::reject(format!("{FIGURE_FELL}: {e}"));
        let figure = payload.get("figure").and_then(Value::as_int).map_err(bad)?;
        let levels = payload.get("levels").and_then(Value::as_int).map_err(bad)?;
        let figure = FigureId(u32::try_from(figure).map_err(|_| Halt::reject("no such figure"))?);
        let beyond = u32::try_from(levels)
            .unwrap_or(0)
            .saturating_sub(self.max_fall);
        let amount = beyond.saturating_mul(self.per_level);
        let fig = ctx.state().figures.get(figure.index());
        // Something earlier in the chain may already have removed the figure.
        if amount == 0 || fig.is_none_or(|f| f.status != FigureStatus::OnBoard) {
            return Ok(());
        }
        wound(ctx, env, figure, amount)
    }
}
