//! `ctx.combat`: attacks for scripts.
//!
//! - `attack { attacker, target, dice, range }` resolves a full attack, windows included, for a
//!   special attack. `dice` replaces the attacker's `attack` stat and `range` its `range` stat;
//!   the target must be in that range and, beyond adjacency, in sight. Unlike the ordinary
//!   attack, it may target any figure, allies included.
//! - `targets(figure, { range })` lists the enemies `figure` may attack, by ID.
//! - `preview(attacker, target, { dice, range })` returns the pools an attack would roll, as
//!   `{ attack_dice, defense_dice, ranged, cover }`, or nil if it may not attack that target.
//! - `current()` returns the attack in progress as `{ attacker, target, ranged, cover,
//!   attack_dice, defense_dice, hits, blocks, wounds }`, or nil.
//! - `set { ... }` changes `attack_dice`, `defense_dice`, `hits` or `blocks` of the attack in
//!   progress, for reactions. Wounds are always hits minus blocks.

use strata_kernel::{Sym, Value, ValueError};
use strata_model::FigureId;
use strata_rules::{ApiHandler, Ctx, Halt, MAX_POOL, NativeEnv};

use super::Rules;
use super::resolve::{self, Record, SETTABLE, record_key};
use super::sight::Sight;

/// Runs the functions of the `combat` namespace.
pub(super) struct Api(pub(super) std::sync::Arc<Rules>);

/// The figure's ID as scripts and action arguments carry it.
pub(crate) fn id_value(figure: FigureId) -> Value {
    Value::Int(i64::from(figure.0))
}

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
        (attacker, target): (FigureId, FigureId),
        range: Option<i64>,
    ) -> Result<Option<Sight>, Halt> {
        Sight::of(&self.0, ctx.state(), env, attacker, target, range)
    }

    fn attack(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, spec: &Value) -> Result<(), Halt> {
        let get = |key: &str| {
            let found = spec.get(key);
            found.map_err(|e| Halt::reject(format!("ctx.combat.attack: {e}")))
        };
        let attacker = figure_of(get("attacker")?, "ctx.combat.attack `attacker`")?;
        let target = figure_of(get("target")?, "ctx.combat.attack `target`")?;
        let (dice, range) = (option(Some(spec), "dice")?, option(Some(spec), "range")?);
        let sight = self.sight(ctx, env, (attacker, target), range)?;
        let sight =
            sight.ok_or_else(|| Halt::reject("ctx.combat.attack: the target is out of reach"))?;
        resolve::attack(&self.0, ctx, env, attacker, target, sight, dice)
    }

    fn preview(&self, ctx: &dyn Ctx, env: &dyn NativeEnv, args: &[Value]) -> Result<Value, Halt> {
        let attacker = figure_of(arg(args, 0, "preview")?, "ctx.combat.preview")?;
        let target = figure_of(arg(args, 1, "preview")?, "ctx.combat.preview")?;
        let (dice, range) = (option(args.get(2), "dice")?, option(args.get(2), "range")?);
        let Some(sight) = self.sight(ctx, env, (attacker, target), range)? else {
            return Ok(Value::Nil);
        };
        let record = resolve::start(&self.0, ctx, env, (attacker, target), sight, dice)?;
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
        for (key, value) in changes.as_map().map_err(bad)? {
            let n = value.as_int().map_err(bad)?;
            if !(0..=MAX_POOL).contains(&n) {
                let range = format!("ctx.combat.set: `{key}` must be 0..={MAX_POOL}");
                return Err(Halt::reject(range));
            }
            let field = match key.as_str() {
                "attack_dice" => &mut record.attack_dice,
                "defense_dice" => &mut record.defense_dice,
                "hits" => &mut record.hits,
                "blocks" => &mut record.blocks,
                other => {
                    let allowed = SETTABLE.join(", ");
                    let message = format!("ctx.combat.set: `{other}` is not one of {allowed}");
                    return Err(Halt::reject(message));
                }
            };
            *field = n;
        }
        let key = record_key();
        let value = record.to_value();
        ctx.emit(strata_model::Event::VarSet { key, value })
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
