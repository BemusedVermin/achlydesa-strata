//! `core:activation`: a unit's turn, as steps and per-figure slots (design §6.3, §6.5).
//!
//! An activation gives each figure on the board one slot per step (`move` then `attack` by
//! default). With `all_figures_per_step` every figure spends the current step before any figure
//! starts the next; without it one figure spends everything before another may begin. The player
//! may end the activation early. Abilities grant extra slots through `ctx.activation.grant`, or
//! from Rust by emitting a [`GRANT_SLOT`] event.
//!
//! The module registers the `activate_unit` and `end_activation` commands and provides its
//! [`ActivationRules`] as a service, which spending slots and listing legal actions build on.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use strata_kernel::{ModuleId, SlotId, Sym, Value, ValueError};
use strata_model::{
    Command, ConfigField, Descriptor, Event, FigureId, Logic, PlayerId, TriggerDef,
};
use strata_rules::{
    ActivationRules, ApiHandler, CommandHandler, ConfigSchema, Ctx, Halt, LuaApi, ModuleError,
    NativeEnv, NativeLogic, OrderingPolicy, Registrar, RuleModule,
};

/// The custom event that grants a slot, carrying `{ figure, slot, extra }`.
pub const GRANT_SLOT: &str = "grant_slot";

/// The name the grant logic is registered under.
const GRANT: &str = "core:activation.grant";

/// The activation slot.
pub struct Activation;

impl RuleModule for Activation {
    fn id(&self) -> ModuleId {
        ModuleId::new("core:activation")
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("activation"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        static SCHEMA: OnceLock<ConfigSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            let field = |kind: &str, default: Value| ConfigField {
                kind: Sym::new(kind),
                default,
            };
            let text = |s: &str| Value::Str(Sym::new(s));
            let steps = Value::List(vec![text("move"), text("attack")]);
            ConfigSchema::new(BTreeMap::from([
                (Sym::new("steps"), field("list", steps)),
                (
                    Sym::new("budgets"),
                    field("map", Value::Map(BTreeMap::new())),
                ),
                (
                    Sym::new("all_figures_per_step"),
                    field("bool", Value::Bool(true)),
                ),
            ]))
        })
    }

    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError> {
        let rules = parse(cfg).map_err(ModuleError::Invalid)?;
        r.service::<ActivationRules>(Box::new(rules.clone()));
        let rules = Arc::new(rules);
        r.command(Activate(rules.clone()));
        r.command(End);
        let functions = ["grant", "end"].map(Sym::new).to_vec();
        let api = Api(rules.clone());
        r.lua_api_native("activation", LuaApi { functions }, Box::new(api));
        r.native(GRANT, Box::new(Grant(rules)));
        let call = [(Sym::new("native"), Value::Str(Sym::new(GRANT)))];
        r.trigger(TriggerDef {
            event: Sym::new(GRANT_SLOT),
            filter: None,
            limit: None,
            effect: Logic::Native(Descriptor(Value::Map(call.into()))),
        });
        Ok(())
    }
}

/// Reads the module's configuration.
fn parse(cfg: &Value) -> Result<ActivationRules, String> {
    let bad = |e: ValueError| e.to_string();
    let list = cfg.get("steps").and_then(Value::as_list).map_err(bad)?;
    let steps: Result<Vec<Sym>, _> = list.iter().map(|v| v.as_str().map(Sym::new)).collect();
    let steps = steps.map_err(bad)?;
    if steps.is_empty() {
        return Err("steps must name at least one step".into());
    }
    let mut budgets = BTreeMap::new();
    for (slot, count) in cfg.get("budgets").and_then(Value::as_map).map_err(bad)? {
        let count = count.as_int().map_err(bad)?;
        let count =
            u8::try_from(count).map_err(|_| format!("budget of `{slot}` is out of range"))?;
        budgets.insert(slot.clone(), count);
    }
    let all_figures = cfg.get("all_figures_per_step").and_then(Value::as_bool);
    let ordering = match all_figures.map_err(bad)? {
        true => OrderingPolicy::AllFiguresPerStep,
        false => OrderingPolicy::PerFigure,
    };
    Ok(ActivationRules {
        steps,
        budgets,
        ordering,
    })
}

/// Handles `activate_unit`: the active player starts a unit's activation.
struct Activate(Arc<ActivationRules>);

impl CommandHandler for Activate {
    fn kind(&self) -> &str {
        "activate_unit"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, player: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::ActivateUnit(unit) = command else {
            return Err(Halt::reject("not an activation"));
        };
        if ctx.state().flow.active != player {
            return Err(Halt::reject("it is not your turn"));
        }
        if !ctx.may_activate(player, *unit) {
            return Err(Halt::reject("that unit may not be activated now"));
        }
        self.0.begin(ctx, player, *unit)
    }
}

/// Handles `end_activation`: the active player gives up whatever slots are left.
struct End;

impl CommandHandler for End {
    fn kind(&self) -> &str {
        "end_activation"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, player: PlayerId, _: &Command) -> Result<(), Halt> {
        end(ctx, player)
    }
}

/// Ends `player`'s activation, however many slots are left.
fn end(ctx: &mut dyn Ctx, player: PlayerId) -> Result<(), Halt> {
    let state = ctx.state();
    if state.flow.active != player {
        return Err(Halt::reject("it is not your turn"));
    }
    if state.flow.activation.is_none() {
        return Err(Halt::reject("no unit is activated"));
    }
    ctx.emit(Event::ActivationEnded)
}

/// Reads `figure, slot, extra` and grants the slots.
fn grant(rules: &ActivationRules, ctx: &mut dyn Ctx, args: &[Value]) -> Result<(), Halt> {
    let bad = |e: ValueError| Halt::reject(format!("grant: {e}"));
    let at = |i: usize| {
        args.get(i)
            .ok_or_else(|| Halt::reject("grant needs a figure, a slot and an amount"))
    };
    let figure = FigureId(at(0)?.as_int().map_err(bad)? as u32);
    let slot = Sym::new(at(1)?.as_str().map_err(bad)?);
    let extra = u8::try_from(at(2)?.as_int().map_err(bad)?);
    let extra = extra.map_err(|_| Halt::reject("grant: the amount is out of range"))?;
    rules.grant(ctx, figure, &slot, extra)
}

/// Runs `ctx.activation.*` for scripts.
struct Api(Arc<ActivationRules>);

impl ApiHandler for Api {
    fn call(
        &self,
        ctx: &mut dyn Ctx,
        _: &dyn NativeEnv,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Halt> {
        match name {
            "grant" => grant(&self.0, ctx, args),
            "end" => {
                let player = ctx.state().flow.active;
                end(ctx, player)
            }
            other => Err(Halt::reject(format!("ctx.activation has no `{other}`"))),
        }
        .map(|()| Value::Nil)
    }
}

/// Grants the slots a [`GRANT_SLOT`] event asks for.
struct Grant(Arc<ActivationRules>);

impl NativeLogic for Grant {
    fn run(&self, ctx: &mut dyn Ctx, _: &dyn NativeEnv, event: &Event) -> Result<(), Halt> {
        let Event::Custom { payload, .. } = event else {
            return Ok(());
        };
        let bad = |e: ValueError| Halt::reject(format!("{GRANT_SLOT}: {e}"));
        let field = |name| payload.get(name).map_err(bad).cloned();
        let args = [field("figure")?, field("slot")?, field("extra")?];
        grant(&self.0, ctx, &args)
    }
}

#[cfg(test)]
mod tests;
