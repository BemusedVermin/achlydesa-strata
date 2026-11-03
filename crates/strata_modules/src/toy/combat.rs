//! `toy:combat`: the Rust implementation of the toy combat slot.
//!
//! The attacker rolls a pool of the `combat` die as large as its `attack` stat, then the target
//! rolls one as large as its `defense` stat. Hits beyond the blocks are wounds, added to the
//! target's tally in the variable `wounds_<target>`.

use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{Descriptor, Event, FigureId, Logic, TriggerDef, VarKey};
use strata_rules::{
    ConfigSchema, Ctx, Halt, ModuleError, NativeEnv, NativeLogic, Registrar, RuleModule, roll_pool,
};

use super::{ATTACK_DECLARED, no_config};

/// The die both pools roll.
const DIE: &str = "combat";
/// The name the resolver is registered under.
const RESOLVE: &str = "toy:combat.resolve";
/// The module the wound tally belongs to. The Lua module writes under the same name.
const TALLY_OWNER: &str = "toy:combat";

/// The combat slot, written in Rust.
pub struct Combat;

impl RuleModule for Combat {
    fn id(&self) -> ModuleId {
        ModuleId::new("toy:combat")
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("combat"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        no_config()
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.native(RESOLVE, Box::new(Resolve));
        let call = [(Sym::new("native"), Value::Str(Sym::new(RESOLVE)))];
        r.trigger(TriggerDef {
            event: Sym::new(ATTACK_DECLARED),
            filter: None,
            limit: None,
            effect: Logic::Native(Descriptor(Value::Map(call.into()))),
        });
        Ok(())
    }
}

/// Resolves a declared attack: attacker's pool first, then the target's.
struct Resolve;

impl NativeLogic for Resolve {
    fn run(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, event: &Event) -> Result<(), Halt> {
        let Event::Custom { payload, .. } = event else {
            return Err(Halt::reject("combat resolves an attack declaration"));
        };
        let figure = |name: &str| {
            let found = payload.get(name).and_then(Value::as_int);
            let found = found.map_err(|e| Halt::reject(format!("attack `{name}`: {e}")))?;
            Ok::<_, Halt>(FigureId(found as u32))
        };
        let (attacker, target) = (figure("attacker")?, figure("target")?);
        let stat = |ctx: &dyn Ctx, who, name| {
            let found = env.stat(ctx.state(), who, name);
            found.map_err(|e| Halt::reject(e.to_string()))
        };
        let pool = stat(ctx, attacker, "attack")?;
        let hits = roll_pool(ctx, env, DIE, pool)?.count("hit");
        let pool = stat(ctx, target, "defense")?;
        let blocks = roll_pool(ctx, env, DIE, pool)?.count("block");
        let key = VarKey {
            module: ModuleId::new(TALLY_OWNER),
            name: Sym::new(&format!("wounds_{}", target.0)),
        };
        let before = ctx.state().vars.get(&key).and_then(|v| v.as_int().ok());
        let wounds = hits.saturating_sub(blocks) as i64;
        let value = Value::Int(before.unwrap_or(0) + wounds);
        ctx.emit(Event::VarSet { key, value })
    }
}
