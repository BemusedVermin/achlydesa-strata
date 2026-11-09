//! Sample points and blockers for one line-of-sight cast (design §7.6).

use std::collections::BTreeMap;

use strata_kernel::{
    Blocker, BlockerFootprint, DIRECTIONS, Hex, LATTICE, Point, Point3,
    lattice::{INSET_DEN, INSET_NUM},
};
use strata_model::FigureId;
use strata_rules::{FigureVolume, LosWorld};

/// Lattice distance from a hex center to a corner of the 80% inset hex, along each axis of the
/// sum of two neighboring directions. A full corner sits at a third of that sum.
const INSET_CORNER: i64 = LATTICE * INSET_NUM / (3 * INSET_DEN);

/// The seven horizontal offsets sampled in each cell: the center and the six inset corners.
fn offsets() -> [Point; 7] {
    let corner = |i: usize| {
        let (a, b) = (DIRECTIONS[i], DIRECTIONS[(i + 1) % 6]);
        let sum = Point::new(i64::from(a.q + b.q), i64::from(a.r + b.r));
        Point::new(sum.q * INSET_CORNER, sum.r * INSET_CORNER)
    };
    [
        Point::new(0, 0),
        corner(0),
        corner(1),
        corner(2),
        corner(3),
        corner(4),
        corner(5),
    ]
}

/// The points a figure can be hit at: three heights, each at the center and the six inset
/// corners of every footprint cell, so 21 points for a one-space figure. Each cell's heights are
/// those of its own prism, so a cell standing a level up is sampled a level up.
pub(super) fn samples(volume: &FigureVolume) -> Vec<Point3> {
    let mut points = Vec::new();
    for cell in &volume.cells {
        let heights = [cell.bottom + 1, (cell.bottom + cell.top) / 2, cell.top - 1];
        let center = Point::center(cell.space.hex());
        for offset in offsets() {
            let xy = Point::new(center.q + offset.q, center.r + offset.r);
            points.extend(heights.map(|z| Point3::new(xy, z)));
        }
    }
    points
}

/// The blockers of every column a figure other than `skip` stands in: the column's own blockers
/// plus one inset prism per figure cell, from the bottom to the top of that cell's hit zone.
pub(super) fn figure_columns(
    world: &dyn LosWorld,
    skip: [FigureId; 2],
) -> BTreeMap<Hex, Vec<Blocker>> {
    let mut columns: BTreeMap<Hex, Vec<Blocker>> = BTreeMap::new();
    let others = world.figures().into_iter().filter(|f| !skip.contains(f));
    for volume in others.filter_map(|f| world.volume(f)) {
        for cell in &volume.cells {
            let hex = cell.space.hex();
            let column = columns
                .entry(hex)
                .or_insert_with(|| world.blockers(hex).to_vec());
            column.push(Blocker {
                footprint: BlockerFootprint::InsetHex,
                z0: cell.bottom,
                z1: cell.top,
            });
        }
    }
    columns
}
