//! Billboard geometry: upright quads that turn with the camera's yaw (design §13.8).

use glam::{Quat, Vec2, Vec3};
use strata_kernel::{Hex, Space};

use super::{PixelScale, SpriteDef};
use crate::layout::{LEVEL_HEIGHT, Orientation};

/// The rotation that turns a quad drawn in the XY plane, facing `+z`, toward a camera at `yaw`.
/// It turns about the vertical axis only, so the quad never tilts.
pub fn billboard_rotation(yaw: f32) -> Quat {
    Quat::from_rotation_y(yaw)
}

/// The corners of a sprite's quad around its foot point, before [`billboard_rotation`]:
/// bottom-left, bottom-right, top-right, top-left. One pixel is
/// [`PixelScale::world_per_pixel`] in size.
pub fn quad_corners(def: &SpriteDef, scale: PixelScale) -> [Vec3; 4] {
    let px = scale.world_per_pixel();
    let (frame, anchor) = (def.frame.as_vec2(), def.anchor.as_vec2());
    let min = Vec2::new(-anchor.x, anchor.y - frame.y) * px;
    let max = Vec2::new(frame.x - anchor.x, anchor.y) * px;
    [
        Vec3::new(min.x, min.y, 0.0),
        Vec3::new(max.x, min.y, 0.0),
        Vec3::new(max.x, max.y, 0.0),
        Vec3::new(min.x, max.y, 0.0),
    ]
}

/// Where a figure's sprite stands: over the middle of the cells it covers, on the surface of
/// its anchor space. A one-cell figure stands on its anchor's center.
pub fn foot_point(orientation: Orientation, anchor: Space, cells: &[Hex]) -> Vec3 {
    let centers: Vec<(f32, f32)> = if cells.is_empty() {
        vec![orientation.center(anchor.hex())]
    } else {
        cells.iter().map(|c| orientation.center(*c)).collect()
    };
    let n = centers.len() as f32;
    let (x, z) = centers
        .iter()
        .fold((0.0, 0.0), |(x, z), c| (x + c.0, z + c.1));
    Vec3::new(x / n, anchor.z as f32 * LEVEL_HEIGHT, z / n)
}
