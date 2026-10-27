//! Exact 3D occlusion of a segment by column contents (design §7.5, §7.6).

use crate::frac::Frac;
use crate::hex::Hex;
use crate::lattice::Point;
use crate::traverse::{clip_inset, traverse};

/// Sub-level units per level, the vertical unit of line-of-sight heights.
pub const SUB_LEVELS: i64 = 12;

/// A lattice point with a height in sub-level units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point3 {
    /// Position in the plane, in lattice units.
    pub xy: Point,
    /// Height in sub-level units.
    pub z: i64,
}

impl Point3 {
    /// Creates the point at `xy` and height `z`.
    pub const fn new(xy: Point, z: i64) -> Point3 {
        Point3 { xy, z }
    }
}

/// A 3D segment between two lattice points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Seg3 {
    /// The start of the segment.
    pub a: Point3,
    /// The end of the segment.
    pub b: Point3,
}

impl Seg3 {
    /// Creates the segment from `a` to `b`.
    pub const fn new(a: Point3, b: Point3) -> Seg3 {
        Seg3 { a, b }
    }

    /// Returns the same segment traversed from `b` to `a`.
    pub const fn reversed(self) -> Seg3 {
        Seg3 {
            a: self.b,
            b: self.a,
        }
    }

    /// Returns the height at parameter `t`.
    fn height_at(self, t: Frac) -> Frac {
        t.affine(self.a.z.into(), (self.b.z - self.a.z).into())
    }
}

/// The horizontal extent of a blocker within its column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockerFootprint {
    /// The whole hex, as for terrain and features.
    FullHex,
    /// The hex shrunk to 80%, as for figure volumes.
    InsetHex,
}

/// A solid occupying the half-open height range `[z0, z1)` of a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Blocker {
    /// The horizontal extent of the solid.
    pub footprint: BlockerFootprint,
    /// Bottom height in sub-level units, inclusive.
    pub z0: i64,
    /// Top height in sub-level units, exclusive.
    pub z1: i64,
}

/// The result of sampling several segments from one origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClearCount {
    /// How many targets had an unblocked segment.
    pub clear: usize,
    /// The unblocked segments, in target order, for the client to draw.
    pub witnesses: Vec<Seg3>,
}

/// Returns whether `seg` passes through the open interior of any blocker.
///
/// `blockers` lists what stands in a column. Columns in `ignore`, normally the
/// endpoint columns, are skipped. Touching a blocker's top, bottom or side
/// does not block.
pub fn blocked<'b, F>(seg: Seg3, blockers: F, ignore: &[Hex]) -> bool
where
    F: Fn(Hex) -> &'b [Blocker],
{
    traverse(seg.a.xy, seg.b.xy).into_iter().any(|crossing| {
        if ignore.contains(&crossing.hex) {
            return false;
        }
        blockers(crossing.hex).iter().any(|blocker| {
            let span = match blocker.footprint {
                BlockerFootprint::FullHex => Some((crossing.t_in, crossing.t_out)),
                BlockerFootprint::InsetHex => clip_inset(crossing.hex, seg.a.xy, seg.b.xy),
            };
            span.is_some_and(|(t_in, t_out)| overlaps(seg, t_in, t_out, blocker))
        })
    })
}

/// Returns whether the segment's heights over `[t_in, t_out]` meet the open range of `blocker`.
fn overlaps(seg: Seg3, t_in: Frac, t_out: Frac, blocker: &Blocker) -> bool {
    let (h_in, h_out) = (seg.height_at(t_in), seg.height_at(t_out));
    let (low, high) = (h_in.min(h_out), h_in.max(h_out));
    high > Frac::new(blocker.z0.into(), 1) && low < Frac::new(blocker.z1.into(), 1)
}

/// Counts the `targets` visible from `origin` and returns the clear segments.
pub fn clear_count<'b, F>(
    origin: Point3,
    targets: &[Point3],
    blockers: F,
    ignore: &[Hex],
) -> ClearCount
where
    F: Fn(Hex) -> &'b [Blocker],
{
    let witnesses: Vec<Seg3> = targets
        .iter()
        .map(|&target| Seg3::new(origin, target))
        .filter(|&seg| !blocked(seg, &blockers, ignore))
        .collect();
    ClearCount {
        clear: witnesses.len(),
        witnesses,
    }
}
