//! Rust logic that a registered trigger runs in place of a Lua script.

use strata_kernel::{DefId, Hex, Space, Sym};
use strata_model::{
    Answer, Decision, DecisionKind, Descriptor, DieKind, Event, FigureDef, FigureDefRef, FigureId,
    GameState, Logic, PlayerId, RelicDef,
};

use crate::engine::{Ctx, Halt};
use crate::flow::PhaseTree;
use crate::movement::Reach;
use crate::query::{LogicError, VoteQuery};

/// The most dice one pool may hold.
pub const MAX_POOL: i64 = 1024;

/// A trigger effect written in Rust.
///
/// A module registers it by name with [`Registrar::native`](crate::Registrar::native). A trigger
/// whose effect is the descriptor `{ native = "<name>" }` runs it, so a Rust module and a Lua
/// module can fill the same seam.
pub trait NativeLogic: Send + Sync + 'static {
    /// Runs for the event that set the trigger off, emitting events through `ctx`. `env` answers
    /// questions about content that `strata_rules` cannot see itself.
    fn run(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, event: &Event) -> Result<(), Halt>;
}

/// What native logic can ask of the loaded content, as `ctx.query` and `ctx.dice` do for scripts.
pub trait NativeEnv: Send + Sync {
    /// A figure's stat with every modifier applied.
    fn stat(&self, state: &GameState, figure: FigureId, stat: &str) -> Result<i64, LogicError>;
    /// The definition ID and faces of the die called `name`.
    fn die(&self, name: &str) -> Option<(DefId, DieKind)>;
    /// The ruleset's phase tree, for logic that moves the game between phases.
    fn phases(&self) -> Option<&PhaseTree> {
        None
    }
    /// The definition ID of the marker called `name`, such as `wound`. `None` if there is none or
    /// the environment cannot tell.
    fn marker(&self, _name: &str) -> Option<DefId> {
        None
    }
    /// The start zone and deployment order of `player`'s scenario slot.
    fn deployment(&self, _state: &GameState, _player: PlayerId) -> Option<DeploySlot> {
        None
    }
    /// Whether a figure can stand at `space`: it is a standable surface of the board.
    fn standable(&self, _space: Space) -> bool {
        false
    }
    /// The relic with this definition ID.
    fn relic(&self, _id: DefId) -> Option<RelicDef> {
        None
    }
    /// The answer to a vote query, such as `is_engaged`, with every modifier applied.
    fn vote(&self, _state: &GameState, query: &VoteQuery) -> Result<bool, LogicError> {
        Err(LogicError(format!(
            "no environment to query `{}`",
            query.key
        )))
    }
    /// The definition of a figure entry, for its height and footprint. `None` if the environment
    /// cannot tell.
    fn figure_def(&self, _figure: &FigureDefRef) -> Option<&FigureDef> {
        None
    }
    /// Runs a relic's `on_claim` effect for `event`, as a trigger's effect would run.
    fn run_effect(&self, _ctx: &mut dyn Ctx, _effect: &Logic, _event: &Event) -> Result<(), Halt> {
        Ok(())
    }
    /// Every space `figure` can reach, as the ruleset's `movement` module defines it.
    fn reachable(&self, _state: &GameState, _figure: FigureId) -> Result<Vec<Reach>, LogicError> {
        Err(LogicError("the ruleset has no `movement` module".into()))
    }
    /// Results that replace the next roll of `count` dice named `die`, for tests that rig the
    /// dice, as in the script host. `None` leaves the roll to the command's random stream.
    fn forced_roll(&self, _die: &str, _count: usize) -> Option<Result<Vec<u32>, String>> {
        None
    }
}

/// Where a player may deploy and when their turn to place comes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeploySlot {
    /// Position in the deployment order; lower places first.
    pub order: u8,
    /// The columns of the player's start zone.
    pub zone: Vec<Hex>,
}

/// The environment of a runner that has no content: every question fails.
pub struct NoEnv;

impl NativeEnv for NoEnv {
    fn stat(&self, _: &GameState, _: FigureId, stat: &str) -> Result<i64, LogicError> {
        Err(LogicError(format!("no environment to query `{stat}`")))
    }

    fn die(&self, _: &str) -> Option<(DefId, DieKind)> {
        None
    }
}

/// The outcome of one pool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roll {
    /// One result per die: a one-based face number, or the value of a range die.
    pub results: Vec<u32>,
    /// The face each result landed on; empty for a range die.
    pub faces: Vec<Sym>,
}

impl Roll {
    /// How many dice landed on `face`.
    pub fn count(&self, face: &str) -> usize {
        self.faces.iter().filter(|f| f.as_str() == face).count()
    }
}

/// Rolls `n` of the die `name` from the command's stream and emits `DiceRolled`, as
/// `ctx.dice.roll` does.
pub fn roll_pool(ctx: &mut dyn Ctx, env: &dyn NativeEnv, name: &str, n: i64) -> Result<Roll, Halt> {
    let (die, kind) = env
        .die(name)
        .ok_or_else(|| Halt::reject(format!("unknown die `{name}`")))?;
    let count = usize::try_from(n).ok().filter(|_| n <= MAX_POOL);
    let count = count.ok_or_else(|| Halt::reject("dice count must be 0..=1024"))?;
    let mut drawn = Vec::with_capacity(count);
    for _ in 0..count {
        drawn.push(match &kind {
            DieKind::Faces(faces) => ctx.rng().roll(faces.len().max(1) as u32),
            DieKind::Range(low, high) => {
                let span = (i64::from(*high) - i64::from(*low) + 1).max(1) as u32;
                (i64::from(*low) + i64::from(ctx.rng().bounded(span))).max(0) as u32
            }
        });
    }
    // The stream is always drawn from, so a rigged roll leaves later rolls where they were.
    let results = match env.forced_roll(name, drawn.len()) {
        Some(forced) => forced.map_err(Halt::reject)?,
        None => drawn,
    };
    let faces = match &kind {
        DieKind::Faces(faces) => results
            .iter()
            .map(|r| faces[*r as usize - 1].clone())
            .collect(),
        DieKind::Range(..) => Vec::new(),
    };
    ctx.emit(Event::DiceRolled {
        die,
        results: results.clone(),
    })?;
    Ok(Roll { results, faces })
}

/// A pool of one die whose size a stat query decides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DicePool {
    /// The die's name.
    pub die: String,
    /// How many dice to roll.
    pub count: i64,
}

impl DicePool {
    /// The pool of `die` as large as `figure`'s `stat`, modifiers included.
    pub fn for_stat(
        env: &dyn NativeEnv,
        state: &GameState,
        die: &str,
        figure: FigureId,
        stat: &str,
    ) -> Result<Self, Halt> {
        let count = env
            .stat(state, figure, stat)
            .map_err(|e| Halt::reject(e.to_string()))?;
        Ok(Self {
            die: die.to_owned(),
            count,
        })
    }

    /// Rolls the pool from the command's stream, emitting `DiceRolled`.
    pub fn roll(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv) -> Result<Roll, Halt> {
        roll_pool(ctx, env, &self.die, self.count)
    }
}

/// Asks `chooser` whether to reroll the die at `index` of `roll`. If they agree, one replacement
/// die is rolled (emitting `DiceRolled`) and takes the old result's place.
pub fn reroll(
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    chooser: PlayerId,
    die: &str,
    roll: &Roll,
    index: usize,
) -> Result<Roll, Halt> {
    if index >= roll.results.len() {
        return Err(Halt::reject("reroll: no die at that index"));
    }
    let ask = Decision {
        prompt: Sym::new("reroll"),
        kind: DecisionKind::YesNo,
    };
    if ctx.choose(chooser, ask)? != Answer::YesNo(true) {
        return Ok(roll.clone());
    }
    let replacement = roll_pool(ctx, env, die, 1)?;
    let mut rerolled = roll.clone();
    rerolled.results[index] = replacement.results[0];
    if let Some(face) = rerolled.faces.get_mut(index) {
        face.clone_from(&replacement.faces[0]);
    }
    Ok(rerolled)
}

/// The name in a `{ native = "<name>" }` descriptor, or `None` for any other logic.
pub fn native_name(logic: &Logic) -> Option<&str> {
    let Logic::Native(Descriptor(value)) = logic else {
        return None;
    };
    let name = value.get("native").ok()?;
    name.as_str().ok()
}

#[cfg(test)]
mod tests;
