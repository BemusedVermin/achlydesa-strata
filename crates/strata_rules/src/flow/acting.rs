//! Who must act next when it is not the active player (design §12.4).

use strata_model::{GameState, PlayerId};

use crate::native::NativeEnv;
use crate::runtime::RulesRuntime;

/// Names the player who must act next while that is not the active player, such as the player
/// placing a unit during setup. A module provides it as a service; without one, or when it names
/// nobody, the active player acts.
pub trait NextActor: Send + Sync {
    /// The player who must act now, or `None` to leave it to the active player.
    fn next_actor(&self, state: &GameState, env: &dyn NativeEnv) -> Option<PlayerId>;
}

/// The player who submits the next command: the one [`NextActor`] names, or the active player.
/// A pending decision is answered by its chooser instead, which callers check first.
pub(crate) fn acting(rt: &RulesRuntime, env: &dyn NativeEnv, state: &GameState) -> PlayerId {
    let named = rt.service::<dyn NextActor>();
    let named = named.and_then(|s| s.next_actor(state, env));
    named.unwrap_or(state.flow.active)
}
