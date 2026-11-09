//! The combat preview interface (design §13.6).
//!
//! A module in the `combat` slot may provide [`CombatPreview`], so the client can show what an
//! attack would roll before it is taken. The preview reads the state and never rolls.

use strata_kernel::Sym;
use strata_model::{FigureId, GameState};

use crate::native::NativeEnv;
use crate::query::LogicError;

/// One side of an attack: how many dice it rolls, of which die, and which face counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewPool {
    /// How many dice the pool rolls.
    pub dice: i64,
    /// The die's name, as [`NativeEnv::die`] looks it up.
    pub die: String,
    /// The face that counts: a hit for the attack, a block for the defense.
    pub face: Sym,
}

/// The pools an attack would roll now, before any reaction changes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttackPools {
    /// The attacker's pool, counting hits.
    pub attack: PreviewPool,
    /// The target's pool, counting blocks.
    pub defense: PreviewPool,
    /// Whether the target is beyond adjacency, so line of sight was needed.
    pub ranged: bool,
    /// Whether the target adds a defense die for cover.
    pub cover: bool,
}

/// What the combat slot tells the client about an attack before it is taken.
pub trait CombatPreview: Send + Sync {
    /// The name of the native action that resolves the attack, so the client can tell which
    /// legal commands are attacks.
    fn action(&self) -> &str;
    /// The pools `attacker` would roll against `target`, or `None` when it may not attack it.
    fn pools(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        attacker: FigureId,
        target: FigureId,
    ) -> Result<Option<AttackPools>, LogicError>;
}
