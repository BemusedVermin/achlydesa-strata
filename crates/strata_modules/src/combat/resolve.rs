//! One attack, from the first window to the `attack` event.

use std::collections::BTreeMap;

use strata_kernel::{ModuleId, Sym, Value, WindowId};
use strata_model::{DieKind, Event, FigureId, VarKey};
use strata_rules::{Ctx, Halt, MAX_POOL, NativeEnv, roll_pool};

use super::sight::{Sight, height_bonus};
use super::{AFTER_DEFENSE_ROLL, AFTER_WOUNDS, BEFORE_ATTACK_ROLL, BEFORE_DEFENSE_ROLL, Rules};

/// The custom event that closes an attack, carrying its final [`Record`] as a map.
pub const ATTACK_EVENT: &str = "attack";
/// The variable of `core:combat` that holds the attack in progress.
pub const RECORD: &str = "attack";

/// The attack in progress, as reactions read and change it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Record {
    pub(crate) attacker: FigureId,
    pub(crate) target: FigureId,
    /// Whether the target was beyond adjacency.
    pub(crate) ranged: bool,
    /// Whether the target had cover.
    pub(crate) cover: bool,
    pub(crate) attack_dice: i64,
    pub(crate) defense_dice: i64,
    pub(crate) hits: i64,
    pub(crate) blocks: i64,
    pub(crate) wounds: i64,
}

/// The fields of a record that `ctx.combat.set` may change.
pub(crate) const SETTABLE: [&str; 4] = ["attack_dice", "defense_dice", "hits", "blocks"];

/// The variable's key.
pub(crate) fn record_key() -> VarKey {
    VarKey {
        module: ModuleId::new("core:combat"),
        name: Sym::new(RECORD),
    }
}

impl Record {
    /// The record as a map, as scripts and the `attack` event see it.
    pub(crate) fn to_value(&self) -> Value {
        let int = Value::Int;
        let fields = [
            ("attacker", int(i64::from(self.attacker.0))),
            ("target", int(i64::from(self.target.0))),
            ("ranged", Value::Bool(self.ranged)),
            ("cover", Value::Bool(self.cover)),
            ("attack_dice", int(self.attack_dice)),
            ("defense_dice", int(self.defense_dice)),
            ("hits", int(self.hits)),
            ("blocks", int(self.blocks)),
            ("wounds", int(self.wounds)),
        ];
        let fields = fields.into_iter().map(|(k, v)| (Sym::new(k), v));
        Value::Map(fields.collect::<BTreeMap<_, _>>())
    }

    /// Reads a record written by [`Record::to_value`].
    pub(crate) fn from_value(value: &Value) -> Result<Self, Halt> {
        let bad = |e: strata_kernel::ValueError| Halt::reject(format!("the attack record: {e}"));
        let int = |k: &str| value.get(k).and_then(Value::as_int).map_err(bad);
        let flag = |k: &str| value.get(k).and_then(Value::as_bool).map_err(bad);
        let figure = |k: &str| int(k).map(|n| FigureId(n as u32));
        Ok(Self {
            attacker: figure("attacker")?,
            target: figure("target")?,
            ranged: flag("ranged")?,
            cover: flag("cover")?,
            attack_dice: int("attack_dice")?,
            defense_dice: int("defense_dice")?,
            hits: int("hits")?,
            blocks: int("blocks")?,
            wounds: int("wounds")?,
        })
    }

    /// The attack in progress, if any.
    pub(crate) fn current(ctx: &dyn Ctx) -> Result<Option<Self>, Halt> {
        let found = ctx.state().vars.get(&record_key());
        found.map(Self::from_value).transpose()
    }

    /// Writes the record as the attack in progress.
    fn save(&self, ctx: &mut dyn Ctx) -> Result<(), Halt> {
        let key = record_key();
        let value = self.to_value();
        ctx.emit(Event::VarSet { key, value })
    }

    /// Reads back the attack in progress after a window, where reactions may have changed it.
    fn reload(ctx: &mut dyn Ctx) -> Result<Self, Halt> {
        let found = Self::current(ctx)?;
        found.ok_or_else(|| Halt::reject("the attack in progress was cleared"))
    }
}

/// Checks that the configured die exists and has the hit and block faces, naming the config key
/// at fault if not.
fn check(rules: &Rules, env: &dyn NativeEnv) -> Result<(), Halt> {
    let bad = |key: &str, e: String| Halt::reject(format!("core:combat `{key}`: {e}"));
    let Some((_, kind)) = env.die(&rules.die) else {
        return Err(bad("die", format!("unknown die `{}`", rules.die)));
    };
    for (key, face) in [("hit", &rules.hit), ("block", &rules.block)] {
        if !matches!(&kind, DieKind::Faces(faces) if faces.contains(face)) {
            let die = &rules.die;
            return Err(bad(key, format!("`{face}` is not a face of die `{die}`")));
        }
    }
    Ok(())
}

/// The record an attack starts from: its pools, before any reaction. `dice` replaces the
/// attacker's `attack` stat, for special attacks.
pub(crate) fn start(
    rules: &Rules,
    ctx: &dyn Ctx,
    env: &dyn NativeEnv,
    (attacker, target): (FigureId, FigureId),
    sight: Sight,
    dice: Option<i64>,
) -> Result<Record, Halt> {
    let state = ctx.state();
    let stat = |figure, name| {
        let found = env.stat(state, figure, name);
        found.map_err(|e| Halt::reject(e.to_string()))
    };
    let attack = match dice {
        Some(dice) => dice,
        None => stat(attacker, "attack")?,
    };
    let (high_attack, high_defense) = height_bonus(rules, state, attacker, target);
    let defense = stat(target, "defense")? + high_defense + i64::from(sight.cover);
    Ok(Record {
        attacker,
        target,
        ranged: sight.ranged,
        cover: sight.cover,
        attack_dice: (attack + high_attack).clamp(0, MAX_POOL),
        defense_dice: defense.clamp(0, MAX_POOL),
        hits: 0,
        blocks: 0,
        wounds: 0,
    })
}

/// Resolves an attack of `attacker` on `target` through every window, then announces it with the
/// `attack` event. The record of an attack it interrupts is restored afterwards.
pub(crate) fn attack(
    rules: &Rules,
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    attacker: FigureId,
    target: FigureId,
    sight: Sight,
    dice: Option<i64>,
) -> Result<(), Halt> {
    check(rules, env)?;
    let outer = ctx.state().vars.get(&record_key()).cloned();
    let mut record = start(rules, ctx, env, (attacker, target), sight, dice)?;
    record.save(ctx)?;

    ctx.window(&WindowId::new(BEFORE_ATTACK_ROLL))?;
    record = Record::reload(ctx)?;
    let roll = roll_pool(ctx, env, &rules.die, record.attack_dice)?;
    record.hits = roll.count(rules.hit.as_str()) as i64;
    record.save(ctx)?;

    ctx.window(&WindowId::new(BEFORE_DEFENSE_ROLL))?;
    record = Record::reload(ctx)?;
    let roll = roll_pool(ctx, env, &rules.die, record.defense_dice)?;
    record.blocks = roll.count(rules.block.as_str()) as i64;
    record.save(ctx)?;

    ctx.window(&WindowId::new(AFTER_DEFENSE_ROLL))?;
    record = Record::reload(ctx)?;
    record.wounds = (record.hits - record.blocks).max(0);
    record.save(ctx)?;
    let wounds = u32::try_from(record.wounds).unwrap_or(u32::MAX);
    if wounds > 0 && ctx.state().standing(record.target).is_some() {
        crate::units::wound(ctx, env, record.target, wounds)?;
    }

    ctx.window(&WindowId::new(AFTER_WOUNDS))?;
    record = Record::reload(ctx)?;
    ctx.emit(Event::Custom {
        id: Sym::new(ATTACK_EVENT),
        payload: record.to_value(),
    })?;
    let value = outer.unwrap_or_default();
    ctx.emit(Event::VarSet {
        key: record_key(),
        value,
    })
}
