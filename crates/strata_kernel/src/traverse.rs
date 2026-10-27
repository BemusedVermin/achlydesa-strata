//! Exact traversal of a segment through the hex grid (design §7.5).

use crate::frac::Frac;
use crate::hex::{DIRECTIONS, Hex};
use crate::lattice::{INSET_DEN, INSET_NUM, LATTICE, Point};

/// Edge half-plane limit of a full hex: the bisector with a neighbor.
const FULL_LIMIT: i64 = LATTICE;

/// Edge half-plane limit of the 80% inset hex.
const INSET_LIMIT: i64 = LATTICE * INSET_NUM / INSET_DEN;

/// Largest lattice distance along `r` from a hex center to its boundary (2/3 of a unit).
const ROW_REACH: i64 = 2 * LATTICE / 3;

/// A stretch of a segment inside one hex's open interior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crossing {
    /// The hex the segment passes through.
    pub hex: Hex,
    /// Segment parameter where it enters the interior.
    pub t_in: Frac,
    /// Segment parameter where it leaves the interior.
    pub t_out: Frac,
}

/// Returns every hex whose open interior segment `a`→`b` passes through, in increasing `t`.
///
/// Touching an edge or corner does not count, and neither does running along
/// an edge. A zero-length segment inside a hex's interior yields that hex with
/// `t_in = 0` and `t_out = 1`.
pub fn traverse(a: Point, b: Point) -> Vec<Crossing> {
    let mut crossings: Vec<Crossing> = candidates(a, b)
        .into_iter()
        .filter_map(|hex| {
            let (t_in, t_out) = clip(hex, a, b, FULL_LIMIT)?;
            Some(Crossing { hex, t_in, t_out })
        })
        .collect();
    crossings.sort_by_key(|c| (c.t_in, c.t_out));
    crossings
}

/// Returns where segment `a`→`b` crosses the open interior of `hex` shrunk to 80%.
pub fn clip_inset(hex: Hex, a: Point, b: Point) -> Option<(Frac, Frac)> {
    clip(hex, a, b, INSET_LIMIT)
}

/// Cyrus–Beck clip of `a`→`b` against the six open edge half-planes of `hex`.
///
/// Each half-plane is `alpha + t * beta < limit`, linear in `t`.
fn clip(hex: Hex, a: Point, b: Point, limit: i64) -> Option<(Frac, Frac)> {
    let from_center = a.sub(Point::center(hex));
    let delta = b.sub(a);
    let (mut lo, mut hi) = (Frac::ZERO, Frac::ONE);
    for direction in DIRECTIONS {
        let alpha = from_center.twice_dot(direction);
        let beta = delta.twice_dot(direction);
        if beta == 0 {
            if alpha >= limit {
                return None;
            }
            continue;
        }
        let bound = Frac::new((limit - alpha).into(), beta.into());
        if beta > 0 {
            hi = hi.min(bound);
        } else {
            lo = lo.max(bound);
        }
    }
    (lo < hi).then_some((lo, hi))
}

/// Returns a superset of the hexes the segment can enter, without floats.
///
/// Hexes are grouped by row. A hex in row `row` only reaches `r` within
/// [`ROW_REACH`] of its center and `q` within one hex of its center, so for
/// each row the portion of the segment in that band bounds the columns to try.
fn candidates(a: Point, b: Point) -> Vec<Hex> {
    let first_row = (a.r.min(b.r) - ROW_REACH).div_euclid(LATTICE);
    let last_row = (a.r.max(b.r) + ROW_REACH).div_euclid(LATTICE);
    let delta = b.sub(a);
    let lattice = i128::from(LATTICE);
    let mut hexes = Vec::new();
    for row in first_row..=last_row {
        let center = row * LATTICE;
        let Some((t0, t1)) = band(a.r, delta.r, center - ROW_REACH, center + ROW_REACH) else {
            continue;
        };
        let q0 = t0.affine(a.q.into(), delta.q.into());
        let q1 = t1.affine(a.q.into(), delta.q.into());
        let first = q0.min(q1).floor().div_euclid(lattice) - 1;
        let last = -(-q0.max(q1).ceil()).div_euclid(lattice) + 1;
        for q in first..=last {
            hexes.push(Hex::new(q as i32, row as i32));
        }
    }
    hexes
}

/// Returns the parameters in `[0, 1]` where `start + t * delta` lies in `[low, high]`.
fn band(start: i64, delta: i64, low: i64, high: i64) -> Option<(Frac, Frac)> {
    if delta == 0 {
        return (low..=high)
            .contains(&start)
            .then_some((Frac::ZERO, Frac::ONE));
    }
    let at = |edge: i64| Frac::new((edge - start).into(), delta.into());
    let (t0, t1) = (at(low).min(at(high)), at(low).max(at(high)));
    let (t0, t1) = (t0.max(Frac::ZERO), t1.min(Frac::ONE));
    (t0 <= t1).then_some((t0, t1))
}
