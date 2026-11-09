//! One reprisal: the enemy left behind rolls, and its wounding faces wound the leaving figure.

use strata_kernel::{Sym, Value};
use strata_model::{Event, FigureId};
use strata_rules::{Ctx, Halt, NativeEnv, roll_pool};

/// The custom event announcing a reprisal, carrying `{ figure, target }`: who rolls, against whom.
pub const REPRISAL: &str = "reprisal";

/// The roll an enemy makes against a figure leaving it.
pub(super) struct Reprisal {
    /// The die rolled.
    pub(super) die: String,
    /// How many of it.
    pub(super) dice: i64,
    /// The face that deals one wound.
    pub(super) wound_on: Sym,
}

impl Reprisal {
    /// Checks that the configured die exists and that `wound_on` is one of its faces, naming the
    /// config key at fault if not.
    fn check(&self, env: &dyn NativeEnv) -> Result<(), Halt> {
        let faces = [("wound_on", &self.wound_on)];
        crate::shared::check_faces(env, "core:engagement", &self.die, &faces)
    }

    /// Announces the reprisal of `enemy` against `target`, rolls it and applies its wounds.
    pub(super) fn resolve(
        &self,
        ctx: &mut dyn Ctx,
        env: &dyn NativeEnv,
        enemy: FigureId,
        target: FigureId,
    ) -> Result<(), Halt> {
        self.check(env)?;
        let payload = [
            (Sym::new("figure"), Value::Int(i64::from(enemy.0))),
            (Sym::new("target"), Value::Int(i64::from(target.0))),
        ];
        ctx.emit(Event::Custom {
            id: Sym::new(REPRISAL),
            payload: Value::Map(payload.into()),
        })?;
        let roll = roll_pool(ctx, env, &self.die, self.dice)?;
        let wounds = u32::try_from(roll.count(self.wound_on.as_str())).unwrap_or(u32::MAX);
        if wounds == 0 {
            return Ok(());
        }
        crate::units::wound(ctx, env, target, wounds)
    }
}
