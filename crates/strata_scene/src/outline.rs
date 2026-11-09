//! Preview geometry: line segments for a figure's line-of-sight volume, the witness rays, rings
//! around flagged spaces and a move's route (design §7.6, §13.6).
//!
//! Positions follow the line-of-sight lattice exactly: axial coordinates scaled by
//! [`LATTICE`], heights in sub-levels. The drawn prism is the volume the rules cast against, not
//! an approximation of the sprite.

use glam::Vec3;
use strata_kernel::lattice::{INSET_DEN, INSET_NUM};
use strata_kernel::{DIRECTIONS, Hex, LATTICE, Point, Point3, SUB_LEVELS, Seg3, Space};

use super::layout::{LEVEL_HEIGHT, Orientation, surface_point};

/// A line segment in world space.
pub type Line = [Vec3; 2];

/// How far marks float above the surface, so they do not flicker against it.
pub const LIFT: f32 = 0.02;

/// The world position of a lattice point at a sub-level height.
pub fn lattice_point(orientation: Orientation, point: Point3) -> Vec3 {
    let scale = LATTICE as f32;
    let (x, z) = orientation.axial(point.xy.q as f32 / scale, point.xy.r as f32 / scale);
    Vec3::new(x, point.z as f32 / SUB_LEVELS as f32 * LEVEL_HEIGHT, z)
}

/// A witness ray, or any line-of-sight segment, in world space.
pub fn segment(orientation: Orientation, seg: Seg3) -> Line {
    [
        lattice_point(orientation, seg.a),
        lattice_point(orientation, seg.b),
    ]
}

/// The six corners of `hex` scaled toward its center by `num / den`, in lattice units: the
/// corner between neighbors `i` and `i + 1` sits at a third of their sum.
fn corners(hex: Hex, num: i64, den: i64) -> [Point; 6] {
    let center = Point::center(hex);
    std::array::from_fn(|i| {
        let (a, b) = (DIRECTIONS[i], DIRECTIONS[(i + 1) % 6]);
        let reach = |n: i32| i64::from(n) * LATTICE * num / (3 * den);
        Point::new(center.q + reach(a.q + b.q), center.r + reach(a.r + b.r))
    })
}

/// The edges of the vertical prism over each of `cells`, inset to 80% of the hex as the rules
/// inset figure volumes, from sub-level `bottom` to `top`: a ring at each end and the six
/// uprights, so 18 lines per cell.
pub fn volume_outline(orientation: Orientation, cells: &[Hex], bottom: i64, top: i64) -> Vec<Line> {
    let mut lines = Vec::with_capacity(cells.len() * 18);
    for &cell in cells {
        let ring = corners(cell, INSET_NUM, INSET_DEN);
        let at = |i: usize, z| lattice_point(orientation, Point3::new(ring[i % 6], z));
        for i in 0..6 {
            lines.push([at(i, bottom), at(i + 1, bottom)]);
            lines.push([at(i, top), at(i + 1, top)]);
            lines.push([at(i, bottom), at(i, top)]);
        }
    }
    lines
}

/// A hexagonal ring on the surface of `space`, `scale` of the way from its center to its
/// corners, lifted just above the surface.
pub fn ring(orientation: Orientation, space: Space, scale: f32) -> Vec<Line> {
    let center = surface_point(orientation, space.hex(), space.z) + Vec3::Y * LIFT;
    let ring = corners(space.hex(), 1, 1);
    let at = |i: usize| {
        let corner = lattice_point(orientation, Point3::new(ring[i % 6], 0));
        let flat = Vec3::new(corner.x, center.y, corner.z);
        center + (flat - center) * scale
    };
    (0..6).map(|i| [at(i), at(i + 1)]).collect()
}

/// The route from `from` through each space of `path`, lifted just above the surfaces.
pub fn route(orientation: Orientation, from: Space, path: &[Space]) -> Vec<Line> {
    let at = |s: &Space| surface_point(orientation, s.hex(), s.z) + Vec3::Y * LIFT;
    let points: Vec<Vec3> = std::iter::once(&from).chain(path).map(at).collect();
    points.windows(2).map(|w| [w[0], w[1]]).collect()
}

#[cfg(test)]
mod tests;
