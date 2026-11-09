//! Picking: the space under the cursor, found by walking a ray through the compiled board.

use glam::Vec3;
use strata_kernel::{Hex, Space};
use strata_model::board::CompiledBoard;

use super::layout::{LEVEL_HEIGHT, Orientation};

/// World distance between samples along the ray.
const STEP: f32 = LEVEL_HEIGHT / 8.0;

/// The board the client draws and picks against.
pub struct SceneBoard {
    /// The compiled map.
    pub board: CompiledBoard,
    /// How its hexes are laid out.
    pub orientation: Orientation,
}

/// Finds the first space the ray reaches.
///
/// The ray is walked from the top of the board down. The first sample inside
/// terrain names the lowest surface above that sample, so side walls and
/// overhangs resolve to the surface the player sees them belonging to, and
/// open ground under a bridge is reached before the bridge is.
pub fn pick(board: &SceneBoard, origin: Vec3, direction: Vec3) -> Option<Space> {
    let dir = direction.try_normalize()?;
    if dir.y >= 0.0 {
        return None;
    }
    let top = board.board.max_level()? as f32 * LEVEL_HEIGHT;
    let mut t = ((top - origin.y) / dir.y).max(0.0);
    let end = -origin.y / dir.y;
    while t <= end {
        let p = origin + dir * t;
        t += STEP;
        let level = (p.y / LEVEL_HEIGHT).floor();
        if level < 0.0 {
            continue;
        }
        let hex = board.orientation.hex_at(p.x, p.z);
        let level = level as u32;
        if !board.board.is_solid(hex, level) {
            continue;
        }
        let surface = board.board.surfaces(hex).iter().find(|s| s.z > level);
        if let Some(surface) = surface {
            return Some(Space::at(hex, surface.z));
        }
    }
    None
}

/// Finds the hex where the ray comes down through level `z`, terrain or not.
///
/// The editor uses this to place on open ground, where [`pick`] finds nothing, and to aim at a
/// locked level. A ray that is not heading down, or starts below the level, finds nothing.
pub fn pick_level(orientation: Orientation, origin: Vec3, direction: Vec3, z: u32) -> Option<Hex> {
    let height = z as f32 * LEVEL_HEIGHT;
    if direction.y >= 0.0 || origin.y < height {
        return None;
    }
    let p = origin + direction * ((height - origin.y) / direction.y);
    Some(orientation.hex_at(p.x, p.z))
}
