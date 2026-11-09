//! The [`CombatPreview`] service: an attack's pools before it is taken (design §13.6).

use std::sync::Arc;

use strata_model::{FigureId, GameState};
use strata_rules::{AttackPools, CombatPreview, Halt, LogicError, NativeEnv, PreviewPool};

use super::sight::{Blind, Sight};
use super::{ACTION, Rules, resolve};
use crate::shared::check_faces;

/// Previews the ordinary attack under the configured rules.
pub(super) struct Previews(pub(super) Arc<Rules>);

impl CombatPreview for Previews {
    fn action(&self) -> &str {
        ACTION
    }

    /// The same pools `ctx.combat.preview` reports, with the die and the faces that count. A die
    /// the attack would reject, unknown or without the `hit` or `block` face, is an error here
    /// too, so the preview never shows odds for an attack that cannot be made.
    fn pools(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        attacker: FigureId,
        target: FigureId,
    ) -> Result<Option<AttackPools>, LogicError> {
        let halted = |h: Halt| match h {
            Halt::Rejected(why) => LogicError(why.0),
            Halt::NeedsDecision { .. } => LogicError("the preview asked for a decision".into()),
        };
        let pair = (attacker, target);
        let rules = &self.0;
        let Some(sight) =
            Sight::of(rules, state, env, pair, None, Blind::Unseen).map_err(halted)?
        else {
            return Ok(None);
        };
        let faces = [("hit", &rules.hit), ("block", &rules.block)];
        check_faces(env, "core:combat", &rules.die, &faces).map_err(halted)?;
        let record = resolve::start(rules, state, env, pair, &sight, None).map_err(halted)?;
        let pool = |dice, face: &strata_kernel::Sym| PreviewPool {
            dice,
            die: rules.die.clone(),
            face: face.clone(),
        };
        Ok(Some(AttackPools {
            attack: pool(record.attack_dice, &rules.hit),
            defense: pool(record.defense_dice, &rules.block),
            ranged: record.ranged,
            cover: record.cover,
            sight: sight.los,
        }))
    }
}
