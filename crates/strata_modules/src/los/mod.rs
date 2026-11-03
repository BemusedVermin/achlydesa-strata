//! `core:los`: the line-of-sight service (design §7.5, §7.6).
//!
//! The `geometric` policy casts from the attacker's target point to every sample point of the
//! defender's volume; the `columns` policy casts one segment from target point to target point.
//! Terrain, blocking features and other figures block; touching a boundary does not.

mod geometry;

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use strata_kernel::{Frac, Hex, ModuleId, Point, Point3, SlotId, Sym, Value, clear_count};
use strata_model::{ConfigField, FigureId};
use strata_rules::{
    ConfigSchema, LineOfSight, LogicError, LosResult, LosWorld, ModuleError, Registrar, RuleModule,
};

#[cfg(test)]
mod tests;

/// How segments are cast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Policy {
    /// Target point to every sample point of the defender's volume.
    Geometric,
    /// Target point to target point.
    Columns,
}

/// How many of the cast segments must be clear for the defender to be visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Threshold {
    /// At least one.
    Any,
    /// At least half.
    Half,
    /// Every one.
    All,
}

/// The line-of-sight slot.
pub struct Los;

/// The service `core:los` provides.
struct Service {
    policy: Policy,
    threshold: Threshold,
}

impl LineOfSight for Service {
    fn query(
        &self,
        world: &dyn LosWorld,
        from: FigureId,
        to: FigureId,
    ) -> Result<LosResult, LogicError> {
        let volume = |f: FigureId| {
            let found = world.volume(f);
            found.ok_or_else(|| LogicError(format!("figure {} is not on the board", f.0)))
        };
        let (seer, seen) = (volume(from)?, volume(to)?);
        let origin = Point3::new(Point::center(seer.anchor), seer.target);
        let targets = match self.policy {
            Policy::Geometric => geometry::samples(&seen),
            Policy::Columns => vec![Point3::new(Point::center(seen.anchor), seen.target)],
        };
        // The endpoint columns never block: each figure stands in its own.
        let ends: Vec<Hex> = seer.cells.iter().chain(&seen.cells).copied().collect();
        let figures = geometry::figure_columns(world, [from, to]);
        let blockers = |hex: Hex| figures.get(&hex).map_or_else(|| world.blockers(hex), |v| v);
        let cast = clear_count(origin, &targets, blockers, &ends);
        let (clear, total) = (cast.clear, targets.len());
        let visible = match self.threshold {
            Threshold::Any => clear >= 1,
            Threshold::Half => 2 * clear >= total,
            Threshold::All => clear == total,
        };
        Ok(LosResult {
            visible,
            clear_fraction: Frac::new(clear as i128, total as i128),
            witnesses: cast.witnesses,
        })
    }
}

fn option(name: &str, default: &str) -> (Sym, ConfigField) {
    let field = ConfigField {
        kind: Sym::new("str"),
        default: Value::Str(Sym::new(default)),
    };
    (Sym::new(name), field)
}

impl RuleModule for Los {
    fn id(&self) -> ModuleId {
        ModuleId::new("core:los")
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("los"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        static SCHEMA: OnceLock<ConfigSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            let fields = [option("policy", "geometric"), option("threshold", "any")];
            ConfigSchema::new(BTreeMap::from(fields))
        })
    }

    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError> {
        let text = |name: &str| {
            let found = cfg.get(name).and_then(Value::as_str);
            found.map_err(|e| ModuleError::Invalid(format!("core:los `{name}`: {e}")))
        };
        let policy = match text("policy")? {
            "geometric" => Policy::Geometric,
            "columns" => Policy::Columns,
            other => {
                return Err(ModuleError::Invalid(format!(
                    "unknown LOS policy `{other}`"
                )));
            }
        };
        let threshold = match text("threshold")? {
            "any" => Threshold::Any,
            "half" => Threshold::Half,
            "all" => Threshold::All,
            other => {
                return Err(ModuleError::Invalid(format!(
                    "unknown LOS threshold `{other}`"
                )));
            }
        };
        let service: Arc<dyn LineOfSight> = Arc::new(Service { policy, threshold });
        r.service(Box::new(service));
        Ok(())
    }
}
