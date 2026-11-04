//! Mapping between axial hexes and levels and the 3D world (design §13.8).

use glam::Vec3;
use strata_kernel::Hex;

/// World units per level; a level is 12 px against a 64 px hex width.
pub const LEVEL_HEIGHT: f32 = 0.325;

const SQRT_3: f32 = 1.732_050_8;

/// How hexes are laid out on the ground plane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    /// Corners point along the depth axis.
    #[default]
    Pointy,
    /// Corners point along the horizontal axis.
    Flat,
}

impl Orientation {
    /// The ground position `(x, z)` of the center of `hex`, with a hex corner one unit from it.
    pub fn center(self, hex: Hex) -> (f32, f32) {
        let (q, r) = (hex.q as f32, hex.r as f32);
        match self {
            Self::Pointy => (SQRT_3 * (q + r / 2.0), 1.5 * r),
            Self::Flat => (1.5 * q, SQRT_3 * (r + q / 2.0)),
        }
    }

    /// The hex containing the ground position `(x, z)`.
    pub fn hex_at(self, x: f32, z: f32) -> Hex {
        let (q, r) = match self {
            Self::Pointy => (SQRT_3 / 3.0 * x - z / 3.0, 2.0 / 3.0 * z),
            Self::Flat => (2.0 / 3.0 * x, -x / 3.0 + SQRT_3 / 3.0 * z),
        };
        round_axial(q, r)
    }
}

/// Rounds fractional axial coordinates to the nearest hex through cube coordinates.
fn round_axial(q: f32, r: f32) -> Hex {
    let s = -q - r;
    let (mut rq, mut rr, rs) = (q.round(), r.round(), s.round());
    let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());
    if dq > dr && dq > ds {
        rq = -rr - rs;
    } else if dr > ds {
        rr = -rq - rs;
    }
    Hex::new(rq as i32, rr as i32)
}

/// The world position of the surface at `hex` and level `z`.
pub fn surface_point(orientation: Orientation, hex: Hex, z: u32) -> Vec3 {
    let (x, zz) = orientation.center(hex);
    Vec3::new(x, z as f32 * LEVEL_HEIGHT, zz)
}
