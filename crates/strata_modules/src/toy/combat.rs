//! `toy:combat`: the Rust implementation of the toy combat slot.
//!
//! The attacker and the defender each roll a pool of the toy pack's one die. The faces are,
//! in order, three hits, two blocks and a blank. Hits beyond the blocks are wounds, added to
//! the target's tally in the variable `wounds_<target>`.

use strata_kernel::{DefId, ModuleId, SlotId, Sym, Value};
use strata_model::{Descriptor, Event, Logic, TriggerDef, VarKey};
use strata_rules::{ConfigSchema, Ctx, Halt, ModuleError, NativeLogic, Registrar, RuleModule};

use super::{ATTACK_DECLARED, no_config};

/// The toy pack has one die, so it is the first of its kind.
const DIE: DefId = DefId(0);
/// The most dice one pool may hold.
const MAX_POOL: usize = 1024;
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

/// Resolves a declared attack: attacker's pool first, then the defender's.
struct Resolve;

impl NativeLogic for Resolve {
    fn run(&self, ctx: &mut dyn Ctx, event: &Event) -> Result<(), Halt> {
        let Event::Custom { payload, .. } = event else {
            return Err(Halt::reject("combat resolves an attack declaration"));
        };
        let field = |name: &str| {
            let found = payload.get(name).and_then(Value::as_int);
            found.map_err(|e| Halt::reject(format!("attack `{name}`: {e}")))
        };
        let (target, attack, defense) = (field("target")?, field("attack")?, field("defense")?);
        let hits = roll(ctx, attack)?.into_iter().filter(|r| *r <= 3).count();
        let blocks = roll(ctx, defense)?
            .into_iter()
            .filter(|r| (4..=5).contains(r))
            .count();
        let key = VarKey {
            module: ModuleId::new(TALLY_OWNER),
            name: Sym::new(&format!("wounds_{target}")),
        };
        let before = ctx.state().vars.get(&key).and_then(|v| v.as_int().ok());
        let wounds = hits.saturating_sub(blocks) as i64;
        let value = Value::Int(before.unwrap_or(0) + wounds);
        ctx.emit(Event::VarSet { key, value })
    }
}

/// Rolls `n` dice from the command's stream and emits `DiceRolled`.
fn roll(ctx: &mut dyn Ctx, n: i64) -> Result<Vec<u32>, Halt> {
    let count = usize::try_from(n).ok().filter(|n| *n <= MAX_POOL);
    let count = count.ok_or_else(|| Halt::reject("a dice pool holds 0 to 1024 dice"))?;
    let results: Vec<u32> = (0..count).map(|_| ctx.rng().roll(6)).collect();
    let die = DIE;
    ctx.emit(Event::DiceRolled {
        die,
        results: results.clone(),
    })?;
    Ok(results)
}
