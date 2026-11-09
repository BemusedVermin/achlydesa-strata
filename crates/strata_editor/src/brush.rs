//! The hexes a freeform brush covers (design §14.2): one hex, a line, or a filled region.

use std::collections::{BTreeSet, VecDeque};

use strata_kernel::Hex;

use crate::occupancy::Occupancy;
use crate::place::Problem;

/// The most hexes one region fill covers; a larger region is refused.
pub const FILL_LIMIT: usize = 4096;

/// A freeform brush.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Brush {
    /// The hex under the cursor. With a multi-hex tile or a feature this places the whole item.
    #[default]
    Single,
    /// Every hex on the line from where the drag started to the cursor.
    Line,
    /// The connected region under the cursor whose top is the same as the cursor's hex; on open
    /// ground, only an enclosed hole.
    Fill,
}

/// Fixed-point scale for [`line`]: large enough that the nudge below never crosses a rounding
/// boundary, small enough that nothing overflows for any board.
const LINE_SCALE: i64 = 16;

/// The hexes on the straight line from `from` to `to`, both included, each adjacent to the next.
///
/// This is the usual cube-coordinate interpolation done in exact integers. The interpolated point
/// is nudged by a tiny fixed amount off the hex edges, so a line along an edge picks the same side
/// every time.
pub fn line(from: Hex, to: Hex) -> Vec<Hex> {
    let n = i64::from(from.distance(to));
    if n == 0 {
        return vec![from];
    }
    let den = n * LINE_SCALE;
    let start = [from.q, from.r, from.s()].map(i64::from);
    let delta = [to.q - from.q, to.r - from.r, to.s() - from.s()].map(i64::from);
    let nudge = [1, 2, -3];
    (0..=n)
        .map(|i| {
            let num: [i64; 3] =
                std::array::from_fn(|k| (start[k] * n + delta[k] * i) * LINE_SCALE + nudge[k]);
            cube_round(num, den)
        })
        .collect()
}

/// Rounds the cube point `num / den` to the hex containing it.
fn cube_round(num: [i64; 3], den: i64) -> Hex {
    let mut rounded = num.map(|v| (2 * v + den).div_euclid(2 * den));
    let off: [i64; 3] = std::array::from_fn(|k| (num[k] - rounded[k] * den).abs());
    if off[0] > off[1] && off[0] > off[2] {
        rounded[0] = -rounded[1] - rounded[2];
    } else if off[1] > off[2] {
        rounded[1] = -rounded[0] - rounded[2];
    }
    // Coordinates come from points between two `i32` hexes, so they fit.
    Hex::new(rounded[0] as i32, rounded[1] as i32)
}

/// The region a fill at `start` covers, in hex order.
///
/// The region is every hex connected to `start` through hexes with the same top: the same
/// terrain at the same level, or no terrain at all.
///
/// Open ground is filled only where it is a hole: a region that never reaches the edge of the
/// axial bounding box of the map's terrain, so terrain closes it in on every side. Anything else
/// (open ground outside the terrain, a bay open to the outside, an empty map) is refused with
/// [`Problem::NotEnclosed`]. A region of more than [`FILL_LIMIT`] hexes is refused with
/// [`Problem::TooLarge`] rather than cut short.
pub fn fill(occupancy: &Occupancy, start: Hex) -> Result<Vec<Hex>, Problem> {
    let key = |hex: Hex| occupancy.top_span(hex).map(|s| (s.z1, s.terrain));
    let target = key(start);
    let bounds = occupancy.bounds();
    let on_edge = |hex: Hex| {
        bounds.is_none_or(|(min, max)| {
            hex.q <= min.q || hex.q >= max.q || hex.r <= min.r || hex.r >= max.r
        })
    };
    let mut region = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(hex) = queue.pop_front() {
        // Open ground on or past the box's edge can lead outside, so it is not a hole. Checking
        // before expanding also keeps an open-ground fill inside the box.
        if target.is_none() && on_edge(hex) {
            return Err(Problem::NotEnclosed);
        }
        for next in hex.neighbors() {
            if !region.contains(&next) && key(next) == target {
                if region.len() >= FILL_LIMIT {
                    return Err(Problem::TooLarge);
                }
                region.insert(next);
                queue.push_back(next);
            }
        }
    }
    Ok(region.into_iter().collect())
}
