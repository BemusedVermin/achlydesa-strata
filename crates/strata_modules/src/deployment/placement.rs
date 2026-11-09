//! Whether a placement is legal: the validator `place_unit` runs, shared with anything that
//! lists legal placements.

use std::collections::BTreeSet;
use std::fmt;

use strata_kernel::{Rotation, Space};
use strata_model::{FigureId, FigureStatus, GameState, PlayerId};
use strata_rules::NativeEnv;

use super::payload::PlaceUnit;

/// Why a figure, or a unit, cannot be placed where it was asked to go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Misplaced {
    /// The unit is someone else's, or does not exist.
    NotYours,
    /// Some figure of the unit is already off reserve.
    AlreadyPlaced,
    /// The payload lists a different number of spots than the unit has figures.
    WrongCount {
        /// The unit's figures.
        want: usize,
        /// The payload's spots.
        got: usize,
    },
    /// The player's scenario slot has no start zone.
    NoStartZone,
    /// A cell of the footprint is outside the start zone.
    OutsideZone,
    /// A cell has no standable surface the figure fits on, within the footprint's spread.
    CannotStand,
    /// A cell is covered by another figure, or by another figure of the same placement.
    Occupied,
}

impl fmt::Display for Misplaced {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotYours => f.write_str("that is not your unit"),
            Self::AlreadyPlaced => f.write_str("that unit is already on the board"),
            Self::WrongCount { want, got } => write!(f, "the unit has {want} figures, not {got}"),
            Self::NoStartZone => f.write_str("you have no start zone"),
            Self::OutsideZone => f.write_str("that space is outside your start zone"),
            Self::CannotStand => f.write_str("a figure cannot stand there"),
            Self::Occupied => f.write_str("that space is occupied"),
        }
    }
}

/// The spaces `figure` would cover if `player` placed it with its anchor on `anchor`, turned by
/// `rotation`.
///
/// Every cell of the turned footprint must be inside `player`'s start zone, on a standable
/// surface the figure fits on as movement would stand it (see `NativeEnv::cells`), and free of
/// every figure already on the board. Turn order and the unit's other figures are left to the
/// caller; [`check_unit`] checks a whole unit.
pub fn check_placement(
    state: &GameState,
    env: &dyn NativeEnv,
    player: PlayerId,
    figure: FigureId,
    anchor: Space,
    rotation: Rotation,
) -> Result<Vec<Space>, Misplaced> {
    let slot = env
        .deployment(state, player)
        .ok_or(Misplaced::NoStartZone)?;
    let body = env.cells(state, figure, anchor, rotation);
    let body = body.ok_or(Misplaced::CannotStand)?;
    if body.iter().any(|s| !slot.zone.contains(&s.hex())) {
        return Err(Misplaced::OutsideZone);
    }
    if !body.iter().all(|s| env.standable(*s)) {
        return Err(Misplaced::CannotStand);
    }
    if body.iter().any(|s| state.occupant(*s).is_some()) {
        return Err(Misplaced::Occupied);
    }
    Ok(body)
}

/// Checks that `player` may place the whole unit as `put` asks: the unit is theirs and in
/// reserve, there is a spot per figure, every figure passes [`check_placement`], and no two of
/// them share a space. Returns each figure with the spaces it covers, in the unit's order.
pub fn check_unit(
    state: &GameState,
    env: &dyn NativeEnv,
    player: PlayerId,
    put: &PlaceUnit,
) -> Result<Vec<(FigureId, Vec<Space>)>, Misplaced> {
    let unit = state.units.get(put.unit.index());
    let unit = unit
        .filter(|u| u.owner == player)
        .ok_or(Misplaced::NotYours)?;
    let reserve = |f: &FigureId| state.figures[f.index()].status == FigureStatus::Reserve;
    if !unit.figures.iter().all(reserve) {
        return Err(Misplaced::AlreadyPlaced);
    }
    if put.at.len() != unit.figures.len() {
        let (want, got) = (unit.figures.len(), put.at.len());
        return Err(Misplaced::WrongCount { want, got });
    }
    let mut used = BTreeSet::new();
    let mut bodies = Vec::with_capacity(put.at.len());
    for (figure, spot) in unit.figures.iter().zip(&put.at) {
        let body = check_placement(state, env, player, *figure, spot.anchor, spot.rotation)?;
        if !body.iter().all(|s| used.insert(*s)) {
            return Err(Misplaced::Occupied);
        }
        bodies.push((*figure, body));
    }
    Ok(bodies)
}
