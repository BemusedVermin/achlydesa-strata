//! Which way a figure faces, and so which sprite direction it shows (design §13.8).

use strata_kernel::{DIRECTIONS, Hex, Rotation};

/// The hex direction a figure faces when its footprint is `footprint` (its cells around the
/// anchor at rotation 0, in footprint order) turned by `rotation`.
///
/// A one-cell figure faces its rotation. A larger figure faces away from its rear, from its other
/// cells toward its anchor, which is its head. When those cells do not point one way (their
/// offsets from the anchor cancel out or fall between directions), it faces away from the first
/// of them, to the nearest direction. For the standard two-cell base `{(0,0), (1,0)}` the facing
/// is `rotation + 3`: movement puts the rear in direction `rotation`, so a snake walks head first.
pub fn facing(footprint: &[Hex], rotation: Rotation) -> Rotation {
    let rear: Vec<Hex> = footprint
        .iter()
        .copied()
        .filter(|c| *c != Hex::ORIGIN)
        .collect();
    let base = match rear.first() {
        None => 0,
        Some(first) => {
            let toward_anchor = rear.iter().fold(Hex::ORIGIN, |sum, c| sum.sub(*c));
            exact_direction(toward_anchor)
                .unwrap_or_else(|| nearest_direction(Hex::ORIGIN.sub(*first)))
        }
    };
    Rotation::from_steps(base + i32::from(rotation.steps()))
}

/// The direction `offset` points along exactly, if it is a whole number of steps along one.
fn exact_direction(offset: Hex) -> Option<i32> {
    let steps = offset.distance(Hex::ORIGIN) as i32;
    let along = |d: &Hex| steps > 0 && d.scale(steps) == offset;
    DIRECTIONS.iter().position(along).map(|d| d as i32)
}

/// The direction closest to `offset`, the lowest on a tie.
fn nearest_direction(offset: Hex) -> i32 {
    let dot = |d: &Hex| d.q * offset.q + d.r * offset.r + d.s() * offset.s();
    let mut best = 0;
    for (d, direction) in DIRECTIONS.iter().enumerate() {
        if dot(direction) > dot(&DIRECTIONS[best]) {
            best = d;
        }
    }
    best as i32
}

#[cfg(test)]
mod tests;
