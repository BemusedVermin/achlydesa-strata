//! `ctx.combat`: attacks for scripts.
//!
//! - `attack { attacker, target, dice, range }` resolves a full attack, windows included, for a
//!   special attack. `dice` (0 to 1024) replaces the attacker's `attack` stat and `range` its
//!   `range` stat; the target must be in that range and, unless adjacent, in sight. Unlike the
//!   ordinary attack, it may target any figure, allies included. It fails while another attack is
//!   in progress, as from a reaction in a combat window: counter-attacks are tracked by
//!   issue #204.
//! - `targets(figure, { range })` lists the enemies `figure` may attack, by ID.
//! - `preview(attacker, target, { dice, range })` returns the pools an attack would roll, as
//!   `{ attack_dice, defense_dice, ranged, cover }`, or nil if it may not attack that target.
//! - `current()` returns the attack in progress as `{ attacker, target, ranged, cover,
//!   attack_dice, defense_dice, hits, blocks, wounds, window, finished }`, or nil. `window` names
//!   the combat window open now.
//! - `set { ... }` changes the attack in progress, for reactions, each value 0 to 1024. Only the
//!   fields the attack still reads may change: `attack_dice` and `defense_dice` in
//!   `combat.before_attack_roll`; `hits` and `defense_dice` in `combat.before_defense_roll`;
//!   `hits` and `blocks` in `combat.after_defense_roll`; nothing in `combat.after_wounds`. Any
//!   other field is an error. Wounds are always hits minus blocks.

use strata_kernel::{Sym, Value, ValueError};
use strata_model::FigureId;
use strata_rules::{ApiHandler, Ctx, Halt, MAX_POOL, NativeEnv};

use super::Rules;
use super::resolve::{self, Record, settable};
use super::sight::{Blind, Sight};
use crate::shared::id_value;

/// Runs the functions of the `combat` namespace.
pub(super) struct Api(pub(super) std::sync::Arc<Rules>);

/// A figure ID from a script or action argument.
pub(crate) fn figure_of(value: &Value, what: &str) -> Result<FigureId, Halt> {
    let id = value
        .as_int()
        .map_err(|e| Halt::reject(format!("{what}: {e}")))?;
    u32::try_from(id)
        .map(FigureId)
        .map_err(|_| Halt::reject(format!("{what}: the figure is out of range")))
}

/// The optional integer `key` of an options table, which may be absent or nil.
fn option(options: Option<&Value>, key: &str) -> Result<Option<i64>, Halt> {
    let bad = |e: ValueError| Halt::reject(format!("ctx.combat: `{key}`: {e}"));
    let Some(options) = options.filter(|o| !o.is_nil()) else {
        return Ok(None);
    };
    match options.as_map().map_err(bad)?.get(key) {
        None | Some(Value::Nil) => Ok(None),
        Some(found) => found.as_int().map(Some).map_err(bad),
    }
}

/// Checks that `n`, given as `what`, is a pool size: 0 to [`MAX_POOL`].
fn pool(what: &str, n: i64) -> Result<i64, Halt> {
    if !(0..=MAX_POOL).contains(&n) {
        return Err(Halt::reject(format!(
            "{what} must be 0..={MAX_POOL}, not {n}"
        )));
    }
    Ok(n)
}

/// The optional `dice` of an options table, which must be a pool size when given.
fn dice_option(options: Option<&Value>, name: &str) -> Result<Option<i64>, Halt> {
    let what = format!("ctx.combat.{name}: `dice`");
    option(options, "dice")?.map(|n| pool(&what, n)).transpose()
}

/// The `n`-th argument, which must be there.
fn arg<'a>(args: &'a [Value], n: usize, name: &str) -> Result<&'a Value, Halt> {
    let found = args.get(n);
    found.ok_or_else(|| Halt::reject(format!("ctx.combat.{name} needs more arguments")))
}

impl Api {
    /// How `attacker` sees `target` within `range`, or `None` when it may not attack it.
    fn sight(
        &self,
        ctx: &dyn Ctx,
        env: &dyn NativeEnv,
        pair: (FigureId, FigureId),
        range: Option<i64>,
        blind: Blind,
    ) -> Result<Option<Sight>, Halt> {
        Sight::of(&self.0, ctx.state(), env, pair, range, blind)
    }

    fn attack(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, spec: &Value) -> Result<(), Halt> {
        let get = |key: &str| {
            let found = spec.get(key);
            found.map_err(|e| Halt::reject(format!("ctx.combat.attack: {e}")))
        };
        let attacker = figure_of(get("attacker")?, "ctx.combat.attack `attacker`")?;
        let target = figure_of(get("target")?, "ctx.combat.attack `target`")?;
        let (dice, range) = (
            dice_option(Some(spec), "attack")?,
            option(Some(spec), "range")?,
        );
        let sight = self.sight(ctx, env, (attacker, target), range, Blind::Reject)?;
        let sight =
            sight.ok_or_else(|| Halt::reject("ctx.combat.attack: the target is out of reach"))?;
        resolve::attack(&self.0, ctx, env, attacker, target, sight, dice)
    }

    fn preview(&self, ctx: &dyn Ctx, env: &dyn NativeEnv, args: &[Value]) -> Result<Value, Halt> {
        let attacker = figure_of(arg(args, 0, "preview")?, "ctx.combat.preview")?;
        let target = figure_of(arg(args, 1, "preview")?, "ctx.combat.preview")?;
        let (dice, range) = (
            dice_option(args.get(2), "preview")?,
            option(args.get(2), "range")?,
        );
        let pair = (attacker, target);
        let Some(sight) = self.sight(ctx, env, pair, range, Blind::Unseen)? else {
            return Ok(Value::Nil);
        };
        let record = resolve::start(&self.0, ctx.state(), env, (attacker, target), &sight, dice)?;
        let fields = [
            ("attack_dice", Value::Int(record.attack_dice)),
            ("defense_dice", Value::Int(record.defense_dice)),
            ("ranged", Value::Bool(record.ranged)),
            ("cover", Value::Bool(record.cover)),
        ];
        Ok(Value::Map(
            fields.into_iter().map(|(k, v)| (Sym::new(k), v)).collect(),
        ))
    }

    fn set(ctx: &mut dyn Ctx, changes: &Value) -> Result<(), Halt> {
        let bad = |e: ValueError| Halt::reject(format!("ctx.combat.set: {e}"));
        let mut record = Record::current(ctx)?
            .ok_or_else(|| Halt::reject("ctx.combat.set: no attack is in progress"))?;
        let allowed = settable(record.window.as_str());
        for (key, value) in changes.as_map().map_err(bad)? {
            if !allowed.contains(&key.as_str()) {
                let window = &record.window;
                let message = match allowed {
                    [] => format!("ctx.combat.set: nothing can change in `{window}`"),
                    _ => format!(
                        "ctx.combat.set: `{key}` cannot change in `{window}`, only {}",
                        allowed.join(", ")
                    ),
                };
                return Err(Halt::reject(message));
            }
            let n = pool(
                &format!("ctx.combat.set: `{key}`"),
                value.as_int().map_err(bad)?,
            )?;
            let field = match key.as_str() {
                "attack_dice" => &mut record.attack_dice,
                "defense_dice" => &mut record.defense_dice,
                "hits" => &mut record.hits,
                _ => &mut record.blocks,
            };
            *field = n;
        }
        record.save(ctx)
    }
}

impl ApiHandler for Api {
    fn call(
        &self,
        ctx: &mut dyn Ctx,
        env: &dyn NativeEnv,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Halt> {
        match name {
            "attack" => self
                .attack(ctx, env, arg(args, 0, name)?)
                .map(|()| Value::Nil),
            "targets" => {
                let figure = figure_of(arg(args, 0, name)?, "ctx.combat.targets")?;
                let range = option(args.get(1), "range")?;
                let found = self.0.targets(ctx.state(), env, figure, range)?;
                Ok(Value::List(found.into_iter().map(id_value).collect()))
            }
            "preview" => self.preview(ctx, env, args),
            "current" => Ok(Record::current(ctx)?.map_or(Value::Nil, |r| r.to_value())),
            "set" => Self::set(ctx, arg(args, 0, name)?).map(|()| Value::Nil),
            other => Err(Halt::reject(format!("ctx.combat has no `{other}`"))),
        }
    }
}
