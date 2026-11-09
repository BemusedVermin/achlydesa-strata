//! `core:movement`: where a figure can move (design §7.3, §7.4).
//!
//! `reachable` runs Dijkstra over (anchor space, rotation) nodes, so footprints of any size move
//! in their `snake` or `rigid` mode with every cell on a surface. Costs, climbing and entry come
//! from the `move_cost`, `can_climb` and `can_enter` queries, asked once per terrain and level
//! change. Stop-on-enter terrain and falls end a move; occupied spaces can be passed through but
//! not stopped on. A space whose route takes the body out of engagement with an enemy, with no
//! cell staying engaged with it, is flagged `provokes_reprisal`. A move that ends in a fall emits
//! `figure_fell`; `fall_wounds` turns on the native falling damage. A move resolves the reprisals
//! its route provokes, through the `engagement` slot, before the figure moves.

mod api;
mod body;
mod fall;
mod search;

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use strata_kernel::{ModuleId, Rotation, SlotId, Space, Sym, Value};
use strata_model::{ConfigField, FigureId};
use strata_rules::{
    ConfigSchema, LogicError, LuaApi, ModuleError, MoveWorld, Movement, Reach, Registrar,
    RuleModule, footprint_cells,
};

pub use self::api::FIGURE_FELL;
use self::body::fits;
use self::fall::Fall;
pub use self::search::{Headroom, Limits, reachable};

#[cfg(test)]
mod footprint_tests;
#[cfg(test)]
mod tests;

/// The movement slot.
pub struct MovementModule;

/// The service `core:movement` provides.
struct Service {
    limits: Limits,
}

impl Movement for Service {
    fn reachable(&self, world: &dyn MoveWorld, figure: FigureId) -> Result<Vec<Reach>, LogicError> {
        reachable(world, figure, self.limits)
    }

    /// Every cell, the anchor's included, on the nearest surface the figure [`fits`] on: the same
    /// test, headroom and all, that a move puts each cell through.
    fn cells(
        &self,
        world: &dyn MoveWorld,
        figure: FigureId,
        anchor: Space,
        rotation: Rotation,
    ) -> Option<Vec<Space>> {
        let height = world.height(figure);
        footprint_cells(world, figure, anchor, rotation, |surface| {
            fits(world.terrain(surface.terrain), self.limits, height, surface)
        })
    }
}

fn field(kind: &str, default: Value) -> ConfigField {
    ConfigField {
        kind: Sym::new(kind),
        default,
    }
}

impl RuleModule for MovementModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("core:movement")
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("movement"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        static SCHEMA: OnceLock<ConfigSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            let headroom = Value::Str(Sym::new("figure_height"));
            ConfigSchema::new(BTreeMap::from([
                (Sym::new("headroom"), field("str", headroom)),
                (Sym::new("max_climb"), field("int", Value::Int(1))),
                (Sym::new("max_fall"), field("int", Value::Int(2))),
                (Sym::new("fall_wounds"), field("int", Value::Int(0))),
            ]))
        })
    }

    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError> {
        let bad = |name: &str, e: &dyn std::fmt::Display| {
            ModuleError::Invalid(format!("core:movement `{name}`: {e}"))
        };
        let headroom = match cfg.get("headroom").and_then(Value::as_str) {
            Ok("ignore") => Headroom::Ignore,
            Ok("figure_height") => Headroom::FigureHeight,
            Ok(other) => return Err(bad("headroom", &format!("unknown value `{other}`"))),
            Err(e) => return Err(bad("headroom", &e)),
        };
        let levels = |name: &str| {
            let n = cfg
                .get(name)
                .and_then(Value::as_int)
                .map_err(|e| bad(name, &e))?;
            u32::try_from(n).map_err(|_| bad(name, &"must not be negative"))
        };
        let limits = Limits {
            headroom,
            max_climb: levels("max_climb")?,
            max_fall: levels("max_fall")?,
        };
        Fall {
            per_level: levels("fall_wounds")?,
            max_fall: limits.max_fall,
        }
        .register(r);
        let service: Arc<dyn Movement> = Arc::new(Service { limits });
        r.service(Box::new(service));
        let functions = ["reachable", "preview", "move"].map(Sym::new).to_vec();
        r.lua_api_native("movement", LuaApi { functions }, Box::new(api::Api));
        Ok(())
    }
}
