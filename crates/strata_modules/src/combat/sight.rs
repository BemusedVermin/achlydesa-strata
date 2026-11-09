//! Whether an attacker can attack a target: range, line of sight, cover and the `can_target` vote.

use strata_kernel::Hex;
use strata_model::{FigureId, GameState};
use strata_rules::{Halt, NativeEnv, VoteQuery};

use super::Rules;
use crate::shared::{height, id_value, overlaps};

/// The stat that says how far a figure attacks.
const RANGE: &str = "range";

/// How a legal attack sees its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Sight {
    /// Whether the target is not adjacent (more than one hex away, or on levels that do not
    /// overlap), so line of sight was needed.
    pub(crate) ranged: bool,
    /// Whether the target adds a defense die for cover.
    pub(crate) cover: bool,
}

/// The columns a figure on the board covers: its footprint, or else the column it stands in.
fn cells(state: &GameState, env: &dyn NativeEnv, figure: FigureId) -> Option<Vec<Hex>> {
    let at = state.standing(figure)?;
    let volume = env.volume(state, figure);
    Some(volume.map_or_else(|| vec![at.hex()], |v| v.cells))
}

/// The hex distance between the nearest cells of two figures on the board.
pub(crate) fn distance(
    state: &GameState,
    env: &dyn NativeEnv,
    a: FigureId,
    b: FigureId,
) -> Option<u32> {
    let (mine, theirs) = (cells(state, env, a)?, cells(state, env, b)?);
    let pairs = mine
        .iter()
        .flat_map(|m| theirs.iter().map(|t| m.distance(*t)));
    pairs.min()
}

/// Whether two figures on the board are adjacent for an attack: their columns at most one hex
/// apart and the levels they span overlapping, as for engagement. Such a target needs no line of
/// sight.
fn adjacent(state: &GameState, env: &dyn NativeEnv, a: FigureId, b: FigureId, apart: u32) -> bool {
    let span = |f: FigureId| {
        let z = state.standing(f)?.z;
        Some((z, height(env, state.figures.get(f.index())?)))
    };
    match (span(a), span(b)) {
        (Some((a, a_height)), Some((b, b_height))) => {
            apart <= 1 && overlaps(a, a_height, b, b_height)
        }
        _ => false,
    }
}

/// What to make of a line-of-sight question that fails, as when the ruleset's `los` slot is
/// empty (design §6.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Blind {
    /// The target is not visible: for listing and previewing targets, so one failing pair does
    /// not hide every other.
    Unseen,
    /// The attack fails, naming both figures: for resolving an attack on the chosen target.
    Reject,
}

impl Sight {
    /// How `attacker` sees `target` when it may attack it within `range` (its `range` stat when
    /// `None`), or `None` when it may not. `blind` says what a failing line-of-sight question
    /// means.
    pub(crate) fn of(
        rules: &Rules,
        state: &GameState,
        env: &dyn NativeEnv,
        (attacker, target): (FigureId, FigureId),
        range: Option<i64>,
        blind: Blind,
    ) -> Result<Option<Self>, Halt> {
        let fail = |e: strata_rules::LogicError| Halt::reject(e.0);
        if attacker == target {
            return Ok(None);
        }
        let Some(apart) = distance(state, env, attacker, target) else {
            return Ok(None);
        };
        let range = match range {
            Some(range) => range,
            None => env.stat(state, attacker, RANGE).map_err(fail)?,
        };
        if i64::from(apart) > range {
            return Ok(None);
        }
        let ranged = !adjacent(state, env, attacker, target, apart);
        let (seen, cover) = if ranged {
            match (env.los(state, attacker, target), blind) {
                // At least half the samples blocked: clear / total <= 1 / 2.
                (Ok(los), _) => {
                    let clear = los.clear_fraction;
                    let cover = rules.cover && 2 * clear.numer() <= clear.denom();
                    (los.visible, cover)
                }
                (Err(_), Blind::Unseen) => (false, false),
                (Err(e), Blind::Reject) => {
                    let (a, t) = (attacker.0, target.0);
                    let message = format!(
                        "core:combat: cannot tell whether figure {a} sees figure {t}, which is \
                         beyond adjacency: {}",
                        e.0
                    );
                    return Err(Halt::reject(message));
                }
            }
        } else {
            (true, false)
        };
        let query = VoteQuery {
            default: seen,
            ..VoteQuery::can_target(id_value(attacker), id_value(target))
        };
        if !env.vote(state, &query).map_err(fail)? {
            return Ok(None);
        }
        Ok(Some(Self { ranged, cover }))
    }
}

/// Dice added for standing higher: to the attack when the attacker is higher, to the defense
/// when the target is. Levels are those of the anchors.
pub(crate) fn height_bonus(
    rules: &Rules,
    state: &GameState,
    attacker: FigureId,
    target: FigureId,
) -> (i64, i64) {
    let level = |f: FigureId| state.standing(f).map(|s| s.z);
    match (level(attacker), level(target)) {
        (Some(a), Some(t)) if a > t => (rules.height_bonus, 0),
        (Some(a), Some(t)) if t > a => (0, rules.height_bonus),
        _ => (0, 0),
    }
}
