//! The engagement service interface (design §6.1, §6.6).
//!
//! A module in the `engagement` slot provides [`Engagement`]: who is engaged with whom, and which
//! enemies get a reprisal when a figure leaves. Movement reads it to flag reachable spaces, and a
//! move calls [`Ctx::leave_engagement`](crate::Ctx::leave_engagement) before it applies.

use strata_kernel::Space;
use strata_model::{FigureId, GameState};

use crate::engine::{Ctx, Halt};
use crate::native::NativeEnv;
use crate::query::LogicError;

/// Who is engaged, and the reprisals for leaving, as the `engagement` slot defines them.
pub trait Engagement: Send + Sync {
    /// The enemies `figure` would be engaged with if it stood on `at`, ordered by ID.
    fn engaged(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        figure: FigureId,
        at: Space,
    ) -> Result<Vec<FigureId>, LogicError>;

    /// The enemies owed a reprisal when `figure`, covering the spaces `before`, steps to cover
    /// `after`, ordered by ID. The body is judged as a whole: an enemy is left only if some space
    /// of `before` is engaged with it and no space of `after` is. A one-cell figure passes one
    /// space on each side.
    fn reprisals(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        figure: FigureId,
        before: &[Space],
        after: &[Space],
    ) -> Result<Vec<FigureId>, LogicError>;

    /// Resolves the reprisals `figure` provokes by walking `path` from where it stands, before the
    /// move applies. The path excludes the start and ends at the destination. The figure may be
    /// out of play when this returns, so the caller checks before moving it.
    fn leave(
        &self,
        ctx: &mut dyn Ctx,
        env: &dyn NativeEnv,
        figure: FigureId,
        path: &[Space],
    ) -> Result<(), Halt>;
}
