//! `core:units`: heroes and squads, wounds, destruction and removal (design §6.1).
//!
//! A figure dies when its wounds reach its `life` stat. The module exports `wound`, `heal`,
//! `destroy` and `remove` to scripts as `ctx.units`, and the same functions to Rust modules.
//! A removed figure leaves the game without being destroyed.

use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{Event, FigureId, FigureStatus, GameState};
use strata_rules::{
    ApiHandler, ConfigSchema, Ctx, Halt, LuaApi, ModuleError, NativeEnv, Registrar, RuleModule,
};

use crate::toy::no_config;

/// The unit kinds this module owns. A unit definition's `kind` is one of these.
pub const KINDS: [&str; 2] = ["hero", "squad"];
/// The marker that counts wounds.
const WOUND: &str = "wound";
/// The stat that says how many wounds a figure survives.
const LIFE: &str = "life";
/// The custom event emitted when a unit's last figure is destroyed or removed, carrying
/// `{ unit }`.
pub const UNIT_REMOVED: &str = "unit_removed";

/// The units slot.
pub struct Units;

impl RuleModule for Units {
    fn id(&self) -> ModuleId {
        ModuleId::new("core:units")
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("units"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        no_config()
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        let functions = ["wound", "heal", "destroy", "remove"];
        let functions = functions.map(Sym::new).to_vec();
        r.lua_api_native("units", LuaApi { functions }, Box::new(Api));
        Ok(())
    }
}

/// Runs `ctx.units.*` for scripts.
struct Api;

impl ApiHandler for Api {
    fn call(
        &self,
        ctx: &mut dyn Ctx,
        env: &dyn NativeEnv,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Halt> {
        let bad = |e: strata_kernel::ValueError| Halt::reject(format!("ctx.units.{name}: {e}"));
        let figure = args
            .first()
            .ok_or_else(|| Halt::reject("a figure is needed"));
        let figure = FigureId(figure?.as_int().map_err(bad)? as u32);
        let amount = || -> Result<u32, Halt> {
            let given = args
                .get(1)
                .ok_or_else(|| Halt::reject("an amount is needed"))?;
            let given = given.as_int().map_err(bad)?;
            u32::try_from(given).map_err(|_| Halt::reject("the amount must not be negative"))
        };
        match name {
            "wound" => wound(ctx, env, figure, amount()?),
            "heal" => heal(ctx, env, figure, amount()?),
            "destroy" => destroy(ctx, figure),
            "remove" => remove(ctx, figure),
            other => Err(Halt::reject(format!("ctx.units has no `{other}`"))),
        }
        .map(|()| Value::Nil)
    }
}

/// Whether the figure is still in play.
fn is_live(status: FigureStatus) -> bool {
    matches!(status, FigureStatus::Reserve | FigureStatus::OnBoard)
}

/// The status of `figure`, which must be in play.
fn require_live(state: &GameState, figure: FigureId) -> Result<(), Halt> {
    match state.figures.get(figure.index()) {
        Some(f) if is_live(f.status) => Ok(()),
        Some(_) => Err(Halt::reject(format!("figure {} is out of play", figure.0))),
        None => Err(Halt::reject(format!("no figure {}", figure.0))),
    }
}

/// The wound marker, which the loaded content must define.
fn wound_marker(env: &dyn NativeEnv) -> Result<strata_kernel::DefId, Halt> {
    let found = env.marker(WOUND);
    found.ok_or_else(|| Halt::reject(format!("the content has no `{WOUND}` marker")))
}

/// Adds `amount` wounds to `figure`, destroying it if they reach its `life`.
pub fn wound(
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    figure: FigureId,
    amount: u32,
) -> Result<(), Halt> {
    require_live(ctx.state(), figure)?;
    if amount == 0 {
        return Err(Halt::reject("a wound must be at least 1"));
    }
    let wound = wound_marker(env)?;
    ctx.emit(Event::WoundsApplied {
        figure,
        wound,
        amount,
    })?;
    // Re-read: a replacement may have changed the amount, or the figure's fate.
    let state = ctx.state();
    let total: i64 = match state.figures.get(figure.index()) {
        Some(f) if is_live(f.status) => f.markers.iter().filter(|m| m.def == wound),
        _ => return Ok(()),
    }
    .map(|m| i64::from(m.count))
    .sum();
    let life = env
        .stat(state, figure, LIFE)
        .map_err(|e| Halt::reject(e.to_string()))?;
    if total >= life {
        destroy(ctx, figure)?;
    }
    Ok(())
}

/// Removes up to `amount` plain wounds, those that no ability placed.
pub fn heal(
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    figure: FigureId,
    amount: u32,
) -> Result<(), Halt> {
    require_live(ctx.state(), figure)?;
    let wound = wound_marker(env)?;
    let markers = &ctx.state().figures[figure.index()].markers;
    let plain = markers
        .iter()
        .find(|m| m.def == wound && m.source.is_none() && m.expiry.is_none());
    let healed = plain.map_or(0, |m| m.count.min(amount));
    if healed == 0 {
        return Ok(());
    }
    ctx.emit(Event::MarkerChanged {
        figure,
        marker: wound,
        delta: -(healed as i32),
        source: None,
        expiry: None,
    })
}

/// Destroys `figure`. When it was its unit's last figure in play, the unit is removed too.
pub fn destroy(ctx: &mut dyn Ctx, figure: FigureId) -> Result<(), Halt> {
    require_live(ctx.state(), figure)?;
    ctx.emit(Event::FigureDestroyed(figure))?;
    unit_left(ctx, figure)
}

/// Removes `figure` from the game without destroying it. When it was its unit's last figure in
/// play, the unit is removed too.
pub fn remove(ctx: &mut dyn Ctx, figure: FigureId) -> Result<(), Halt> {
    require_live(ctx.state(), figure)?;
    ctx.emit(Event::FigureRemoved(figure))?;
    unit_left(ctx, figure)
}

/// Emits `unit_removed` when `figure`, just taken out of play, was its unit's last figure.
fn unit_left(ctx: &mut dyn Ctx, figure: FigureId) -> Result<(), Halt> {
    let state = ctx.state();
    let unit = state.figures[figure.index()].unit;
    let alive = state.units[unit.index()]
        .figures
        .iter()
        .any(|f| is_live(state.figures[f.index()].status));
    if alive {
        return Ok(());
    }
    let payload = [(Sym::new("unit"), Value::Int(i64::from(unit.0)))];
    ctx.emit(Event::Custom {
        id: Sym::new(UNIT_REMOVED),
        payload: Value::Map(payload.into()),
    })
}
