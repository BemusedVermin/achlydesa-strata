//! One attack, from the first window to the `attack` event.

use std::collections::BTreeMap;

use strata_kernel::{ModuleId, Sym, Value, WindowId};
use strata_model::{Event, FigureId, GameState, VarKey};
use strata_rules::{Ctx, DicePool, Halt, MAX_POOL, NativeEnv};

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
    /// The window open now, or the last one the attack opened.
    pub(crate) window: Sym,
    /// Whether the attack ran through every window; false when it ended early because the
    /// attacker left play.
    pub(crate) finished: bool,
}

/// The fields of a record that `ctx.combat.set` may change in `window`: those the attack has not
/// yet read for good. A roll replaces its own count, so `hits` and `blocks` are settable only
/// once rolled, and nothing is settable once the wounds are applied.
pub(crate) fn settable(window: &str) -> &'static [&'static str] {
    match window {
        BEFORE_ATTACK_ROLL => &["attack_dice", "defense_dice"],
        BEFORE_DEFENSE_ROLL => &["hits", "defense_dice"],
        AFTER_DEFENSE_ROLL => &["hits", "blocks"],
        _ => &[],
    }
}

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
            ("window", Value::Str(self.window.clone())),
            ("finished", Value::Bool(self.finished)),
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
            window: Sym::new(value.get("window").and_then(Value::as_str).map_err(bad)?),
            finished: flag("finished")?,
        })
    }

    /// The attack in progress, if any.
    pub(crate) fn current(ctx: &dyn Ctx) -> Result<Option<Self>, Halt> {
        let found = ctx.state().vars.get(&record_key());
        found.map(Self::from_value).transpose()
    }

    /// Writes the record as the attack in progress.
    pub(crate) fn save(&self, ctx: &mut dyn Ctx) -> Result<(), Halt> {
        let key = record_key();
        let value = self.to_value();
        ctx.emit(Event::VarSet { key, value })
    }

    /// Opens `window` with this record saved as the attack in progress, then reads it back
    /// with whatever reactions changed.
    fn window(&mut self, ctx: &mut dyn Ctx, window: &str) -> Result<(), Halt> {
        self.window = Sym::new(window);
        self.save(ctx)?;
        ctx.window(&WindowId::new(window))?;
        let found = Self::current(ctx)?;
        *self = found.ok_or_else(|| Halt::reject("the attack in progress was cleared"))?;
        Ok(())
    }
}

/// The record an attack starts from: its pools, before any reaction. `dice` replaces the
/// attacker's `attack` stat, for special attacks.
pub(crate) fn start(
    rules: &Rules,
    state: &GameState,
    env: &dyn NativeEnv,
    (attacker, target): (FigureId, FigureId),
    sight: &Sight,
    dice: Option<i64>,
) -> Result<Record, Halt> {
    let pool = |figure, stat| DicePool::for_stat(env, state, &rules.die, figure, stat);
    let attack = match dice {
        Some(dice) => dice,
        None => pool(attacker, "attack")?.count,
    };
    let (high_attack, high_defense) = height_bonus(rules, state, attacker, target);
    let defense = pool(target, "defense")?.count + high_defense + i64::from(sight.cover);
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
        window: Sym::new(BEFORE_ATTACK_ROLL),
        finished: false,
    })
}

/// Rolls `count` of the configured die and counts the dice showing `face`.
fn roll(
    rules: &Rules,
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    count: i64,
    face: &Sym,
) -> Result<i64, Halt> {
    let die = rules.die.clone();
    let rolled = DicePool { die, count }.roll(ctx, env)?;
    Ok(rolled.count(face.as_str()) as i64)
}

/// Resolves an attack of `attacker` on `target` through every window, then announces it with the
/// `attack` event. An attack cannot start while another is in progress; counter-attacks are
/// tracked by issue #204.
pub(crate) fn attack(
    rules: &Rules,
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    attacker: FigureId,
    target: FigureId,
    sight: Sight,
    dice: Option<i64>,
) -> Result<(), Halt> {
    if Record::current(ctx)?.is_some() {
        return Err(Halt::reject(
            "core:combat: an attack cannot start while another is in progress \
             (counter-attacks are not supported yet, see issue #204)",
        ));
    }
    let faces = [("hit", &rules.hit), ("block", &rules.block)];
    crate::shared::check_faces(env, "core:combat", &rules.die, &faces)?;
    let record = start(rules, ctx.state(), env, (attacker, target), &sight, dice)?;
    let record = steps(rules, ctx, env, record)?;
    ctx.emit(Event::Custom {
        id: Sym::new(ATTACK_EVENT),
        payload: record.to_value(),
    })?;
    ctx.emit(Event::VarSet {
        key: record_key(),
        value: Value::Nil,
    })
}

/// Runs the windows, rolls and wounds of an attack, and returns its final record. The attack
/// ends early, unfinished, when the attacker has left play after any window before the wounds.
fn steps(
    rules: &Rules,
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    mut record: Record,
) -> Result<Record, Halt> {
    let gone = |ctx: &dyn Ctx, record: &Record| ctx.state().standing(record.attacker).is_none();

    record.window(ctx, BEFORE_ATTACK_ROLL)?;
    if gone(ctx, &record) {
        return Ok(record);
    }
    record.hits = roll(rules, ctx, env, record.attack_dice, &rules.hit)?;

    record.window(ctx, BEFORE_DEFENSE_ROLL)?;
    if gone(ctx, &record) {
        return Ok(record);
    }
    record.blocks = roll(rules, ctx, env, record.defense_dice, &rules.block)?;

    record.window(ctx, AFTER_DEFENSE_ROLL)?;
    if gone(ctx, &record) {
        return Ok(record);
    }
    // Nothing is settable after this window, so the record keeps wounds = hits - blocks.
    record.wounds = (record.hits - record.blocks).max(0);
    let wounds = u32::try_from(record.wounds).unwrap_or(u32::MAX);
    if wounds > 0 && ctx.state().standing(record.target).is_some() {
        crate::units::wound(ctx, env, record.target, wounds)?;
    }

    record.finished = true;
    let applied = Record {
        window: Sym::new(AFTER_WOUNDS),
        ..record.clone()
    };
    record.window(ctx, AFTER_WOUNDS)?;
    // Nothing is settable in the last window: the record announced is the one the wounds came
    // from, so its wounds are always hits minus blocks.
    Ok(applied)
}
