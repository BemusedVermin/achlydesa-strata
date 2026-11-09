//! `core:combat`: the standard attack (design §6.1, §6.7, §7.6).
//!
//! The `attack` action targets an enemy within the attacker's `range` stat, measured in hexes
//! between the nearest cells of the two footprints. An adjacent enemy, at most one hex away on
//! levels that overlap as for `core:engagement`, needs no line of sight; any other, even one
//! straight above or below, needs the `los` slot to see it. Where the `los` slot cannot answer,
//! such an enemy is not listed as a target, and attacking it fails. The `can_target` vote, with
//! that answer as its default, has the last word. The attacker rolls its `attack` stat in dice of `die`, then the target rolls
//! its `defense` stat; each `hit` beyond the `block`s is a wound, applied through `core:units`.
//!
//! The figure standing on the higher level adds `height_bonus` dice to its roll. With `cover`
//! on, a target beyond adjacency adds one defense die when at least half of its line-of-sight
//! samples are blocked.
//!
//! The attack pauses at four windows: `combat.before_attack_roll`, `combat.before_defense_roll`,
//! `combat.after_defense_roll` and `combat.after_wounds`. While it runs, the attack in progress
//! is the variable `attack` of `core:combat`, which reactions read with `ctx.combat.current()`
//! and change with `ctx.combat.set { ... }`. If the attacker leaves play in one of the first three
//! windows, the attack ends there, with no further rolls and no wounds. Once it ends, the custom
//! event `attack` carries the final record. Special attacks resolve through
//! `ctx.combat.attack { ... }`.
//!
//! The module also provides the [`CombatPreview`] service: the pools an attack would roll, with
//! the die and the counted face of each, for the client's attack preview (design §13.6).

mod api;
mod preview;
mod resolve;
mod sight;

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use strata_kernel::{ModuleId, SlotId, Sym, Value, WindowId};
use strata_model::{ConfigField, FigureId, GameState};
use strata_rules::{
    CombatPreview, ConfigSchema, Ctx, Halt, LuaApi, ModuleError, NativeAction, NativeEnv,
    Registrar, RuleModule,
};

pub use self::resolve::{ATTACK_EVENT, RECORD};
use self::sight::{Blind, Sight};
use crate::shared::id_value;

#[cfg(test)]
mod tests;

/// The window before the attacker rolls.
pub const BEFORE_ATTACK_ROLL: &str = "combat.before_attack_roll";
/// The window between the attack roll and the defense roll.
pub const BEFORE_DEFENSE_ROLL: &str = "combat.before_defense_roll";
/// The window after the defense roll, before wounds are applied.
pub const AFTER_DEFENSE_ROLL: &str = "combat.after_defense_roll";
/// The window after wounds are applied.
pub const AFTER_WOUNDS: &str = "combat.after_wounds";
/// Every window an attack opens, in order.
pub const WINDOWS: [&str; 4] = [
    BEFORE_ATTACK_ROLL,
    BEFORE_DEFENSE_ROLL,
    AFTER_DEFENSE_ROLL,
    AFTER_WOUNDS,
];

/// The name the attack action is registered under.
pub const ACTION: &str = "core:combat.attack";
/// The module the combat slot depends on to apply wounds.
const UNITS: &str = "core:units";

/// The combat slot.
pub struct CombatModule;

/// The configured rules of combat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Rules {
    /// The die both pools roll.
    pub(crate) die: String,
    /// The face that scores a hit.
    pub(crate) hit: Sym,
    /// The face that scores a block.
    pub(crate) block: Sym,
    /// Dice the figure on the higher level adds to its roll.
    pub(crate) height_bonus: i64,
    /// Whether a mostly hidden target adds one defense die.
    pub(crate) cover: bool,
}

impl Rules {
    /// The figures `attacker` may attack within `range`, or within its `range` stat when `None`:
    /// enemies on the board it reaches and sees, by ID. An enemy whose line of sight cannot be
    /// answered counts as unseen.
    pub(crate) fn targets(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        attacker: FigureId,
        range: Option<i64>,
    ) -> Result<Vec<FigureId>, Halt> {
        let Some(me) = state.figures.get(attacker.index()) else {
            return Ok(Vec::new());
        };
        if state.standing(attacker).is_none() {
            return Ok(Vec::new());
        }
        let team = state.team(me.controller);
        let mut found = Vec::new();
        for (index, other) in (0u32..).zip(&state.figures) {
            let id = FigureId(index);
            if state.team(other.controller) == team || state.standing(id).is_none() {
                continue;
            }
            if Sight::of(self, state, env, (attacker, id), range, Blind::Unseen)?.is_some() {
                found.push(id);
            }
        }
        Ok(found)
    }
}

/// The ordinary attack, on the `attack` slot.
struct Attack(Arc<Rules>);

impl NativeAction for Attack {
    fn targets(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        figure: FigureId,
    ) -> Result<Vec<Value>, Halt> {
        let found = self.0.targets(state, env, figure, None)?;
        Ok(found.into_iter().map(id_value).collect())
    }

    fn resolve(
        &self,
        ctx: &mut dyn Ctx,
        env: &dyn NativeEnv,
        figure: FigureId,
        args: &Value,
    ) -> Result<(), Halt> {
        let target = api::figure_of(args, "attack")?;
        let pair = (figure, target);
        let sight = Sight::of(&self.0, ctx.state(), env, pair, None, Blind::Reject)?;
        let sight = sight.ok_or_else(|| Halt::reject("that figure cannot attack that target"))?;
        resolve::attack(&self.0, ctx, env, figure, target, sight, None)
    }
}

fn field(kind: &str, default: Value) -> ConfigField {
    ConfigField {
        kind: Sym::new(kind),
        default,
    }
}

impl RuleModule for CombatModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("core:combat")
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("combat"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        static DEPENDS: OnceLock<[ModuleId; 1]> = OnceLock::new();
        DEPENDS.get_or_init(|| [ModuleId::new(UNITS)])
    }

    fn config_schema(&self) -> &ConfigSchema {
        static SCHEMA: OnceLock<ConfigSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            let text = |s: &str| field("str", Value::Str(Sym::new(s)));
            ConfigSchema::new(BTreeMap::from([
                (Sym::new("die"), text("combat")),
                (Sym::new("hit"), text("hit")),
                (Sym::new("block"), text("block")),
                (Sym::new("height_bonus"), field("int", Value::Int(0))),
                (Sym::new("cover"), field("bool", Value::Bool(false))),
            ]))
        })
    }

    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError> {
        let bad = |name: &str, e: &dyn std::fmt::Display| {
            ModuleError::Invalid(format!("core:combat `{name}`: {e}"))
        };
        let text = |name: &str| {
            let found = cfg.get(name).and_then(Value::as_str);
            found.map_err(|e| bad(name, &e))
        };
        let height_bonus = cfg.get("height_bonus").and_then(Value::as_int);
        let height_bonus = height_bonus.map_err(|e| bad("height_bonus", &e))?;
        if !(0..=strata_rules::MAX_POOL).contains(&height_bonus) {
            let range = format!("must be 0..={}", strata_rules::MAX_POOL);
            return Err(bad("height_bonus", &range));
        }
        let cover = cfg.get("cover").and_then(Value::as_bool);
        let rules = Arc::new(Rules {
            die: text("die")?.to_owned(),
            hit: Sym::new(text("hit")?),
            block: Sym::new(text("block")?),
            height_bonus,
            cover: cover.map_err(|e| bad("cover", &e))?,
        });
        for window in WINDOWS {
            r.window(WindowId::new(window));
        }
        r.native_action(
            "attack",
            &["attack"],
            ACTION,
            Box::new(Attack(rules.clone())),
        );
        let previews: Arc<dyn CombatPreview> = Arc::new(preview::Previews(rules.clone()));
        r.service(Box::new(previews));
        let functions = ["attack", "targets", "preview", "current", "set"];
        let functions = functions.map(Sym::new).to_vec();
        r.lua_api_native("combat", LuaApi { functions }, Box::new(api::Api(rules)));
        Ok(())
    }
}
