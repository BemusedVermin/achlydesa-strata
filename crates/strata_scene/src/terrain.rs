//! Terrain mesh generation: hex prisms with only their exposed faces (design §13.3, §13.8).

use std::collections::{BTreeMap, BTreeSet};
use std::f32::consts::FRAC_PI_3;

use glam::Vec3;
use strata_kernel::{DefId, Hex};

use super::layout::{LEVEL_HEIGHT, Orientation};
use super::pick::SceneBoard;

/// Hexes per chunk edge.
const CHUNK_SIZE: i32 = 8;

/// Identifies one square block of hexes that is meshed and rebuilt as a unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChunkKey {
    /// Chunk column.
    pub q: i32,
    /// Chunk row.
    pub r: i32,
}

/// The chunk that contains `hex`.
pub fn chunk_of(hex: Hex) -> ChunkKey {
    ChunkKey {
        q: hex.q.div_euclid(CHUNK_SIZE),
        r: hex.r.div_euclid(CHUNK_SIZE),
    }
}

/// Every chunk that holds part of the board.
pub fn chunks(board: &SceneBoard) -> BTreeSet<ChunkKey> {
    board.board.hexes().map(chunk_of).collect()
}

/// What a mesh is made of; each material gets its own mesh per chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Material {
    /// A terrain definition.
    Terrain(DefId),
    /// The placed feature with this index in the map.
    Feature(u32),
}

/// What to leave out of a chunk's meshes.
#[derive(Clone, Copy, Debug, Default)]
pub struct MeshOptions<'a> {
    /// Levels at or above this are hidden; the editor's level-slicing hook.
    pub hide_from: Option<u32>,
    /// Placed features that have been removed during play.
    pub removed: Option<&'a BTreeSet<u32>>,
}

/// The chunks holding any span of the features in `indices`.
pub fn feature_chunks(board: &SceneBoard, indices: &BTreeSet<u32>) -> BTreeSet<ChunkKey> {
    let holds = |hex: &Hex| {
        let column = board.board.column(*hex);
        column.is_some_and(|c| c.features.iter().any(|f| indices.contains(&f.feature)))
    };
    board.board.hexes().filter(holds).map(chunk_of).collect()
}

/// Flat-shaded triangle soup for one terrain material.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals, one per face.
    pub normals: Vec<[f32; 3]>,
    /// Texture coordinates.
    pub uvs: Vec<[f32; 2]>,
    /// Triangle indices, counter-clockwise seen from outside.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// The number of triangles.
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    fn quad(&mut self, corners: [Vec3; 4], normal: Vec3) {
        let base = self.positions.len() as u32;
        let flip = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .dot(normal)
            < 0.0;
        let sides = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
        for (corner, side_uv) in corners.iter().zip(sides) {
            self.positions.push(corner.to_array());
            self.normals.push(normal.to_array());
            // Tops and undersides are textured from the ground position so the hex tiles seamlessly.
            let uv = if normal.y.abs() > 0.5 {
                [corner.x, corner.z]
            } else {
                side_uv
            };
            self.uvs.push(uv);
        }
        let order = if flip {
            [0, 2, 1, 0, 3, 2]
        } else {
            [0, 1, 2, 0, 2, 3]
        };
        self.indices.extend(order.iter().map(|i| base + i));
    }
}

type Span = (u32, u32);

/// The solid spans of `hex` that are still standing, clipped to the visible levels.
fn spans(board: &SceneBoard, hex: Hex, options: MeshOptions) -> Vec<(Span, Material)> {
    let Some(column) = board.board.column(hex) else {
        return Vec::new();
    };
    let cap = options.hide_from.unwrap_or(u32::MAX);
    let removed = |index: &u32| options.removed.is_some_and(|r| r.contains(index));
    let terrain = column
        .segments
        .iter()
        .map(|s| ((s.z0, s.z1), Material::Terrain(s.terrain)));
    let features = column
        .features
        .iter()
        .filter(|f| !removed(&f.feature))
        .map(|f| ((f.z0, f.z1), Material::Feature(f.feature)));
    terrain
        .chain(features)
        .filter(|((z0, _), _)| *z0 < cap)
        .map(|((z0, z1), material)| ((z0, z1.min(cap)), material))
        .collect()
}

/// The parts of `[z0, z1)` not covered by any of `covers`.
fn uncovered((z0, z1): Span, covers: &[(Span, Material)]) -> Vec<Span> {
    let mut open = vec![(z0, z1)];
    for ((c0, c1), _) in covers {
        open = open
            .into_iter()
            .flat_map(|(a, b)| [(a, b.min(*c0)), (a.max(*c1), b)])
            .filter(|(a, b)| a < b)
            .collect();
    }
    open
}

fn corners(orientation: Orientation, hex: Hex) -> [(f32, f32); 6] {
    let (cx, cz) = orientation.center(hex);
    let base = match orientation {
        Orientation::Pointy => FRAC_PI_3 / 2.0,
        Orientation::Flat => 0.0,
    };
    std::array::from_fn(|k| {
        let angle = base + FRAC_PI_3 * k as f32;
        (cx + angle.cos(), cz + angle.sin())
    })
}

/// The index of the first corner of the side of `hex` facing `neighbor`, and the side's outward normal.
fn side(orientation: Orientation, hex: Hex, neighbor: Hex) -> (usize, Vec3) {
    let (cx, cz) = orientation.center(hex);
    let (nx, nz) = orientation.center(neighbor);
    let (dx, dz) = (nx - cx, nz - cz);
    let base = match orientation {
        Orientation::Pointy => FRAC_PI_3 / 2.0,
        Orientation::Flat => 0.0,
    };
    let k = ((dz.atan2(dx) - base - FRAC_PI_3 / 2.0) / FRAC_PI_3).round() as i32;
    (k.rem_euclid(6) as usize, Vec3::new(dx, 0.0, dz).normalize())
}

fn level_y(z: u32) -> f32 {
    z as f32 * LEVEL_HEIGHT
}

/// Builds the meshes of one chunk, one per terrain material.
///
/// A top face is emitted where nothing sits on the segment, an underside where nothing
/// holds it up (an overhang or a bridge deck), and a side wall for the part of the
/// segment its neighbor does not cover. Terrain and feature spans hide each other's shared
/// faces. `options` drops hidden levels and removed features.
pub fn chunk_meshes(
    board: &SceneBoard,
    chunk: ChunkKey,
    options: MeshOptions,
) -> BTreeMap<Material, MeshData> {
    let mut out: BTreeMap<Material, MeshData> = BTreeMap::new();
    let o = board.orientation;
    for hex in board.board.hexes().filter(|h| chunk_of(*h) == chunk) {
        let mine = spans(board, hex, options);
        let ring = corners(o, hex);
        let around: Vec<_> = hex
            .neighbors()
            .iter()
            .map(|n| (*n, spans(board, *n, options)))
            .collect();
        for &((z0, z1), material) in &mine {
            let mesh = out.entry(material).or_default();
            let covered = |z: u32| mine.iter().any(|((a, b), _)| (*a..*b).contains(&z));
            let at = |k: usize, z: u32| Vec3::new(ring[k % 6].0, level_y(z), ring[k % 6].1);
            if !covered(z1) {
                mesh.quad([at(0, z1), at(1, z1), at(2, z1), at(3, z1)], Vec3::Y);
                mesh.quad([at(0, z1), at(3, z1), at(4, z1), at(5, z1)], Vec3::Y);
            }
            if z0 > 0 && !covered(z0 - 1) {
                mesh.quad([at(0, z0), at(1, z0), at(2, z0), at(3, z0)], Vec3::NEG_Y);
                mesh.quad([at(0, z0), at(3, z0), at(4, z0), at(5, z0)], Vec3::NEG_Y);
            }
            for (neighbor, theirs) in &around {
                let (k, normal) = side(o, hex, *neighbor);
                for (a, b) in uncovered((z0, z1), theirs) {
                    mesh.quad([at(k, a), at(k + 1, a), at(k + 1, b), at(k, b)], normal);
                }
            }
        }
    }
    out.retain(|_, mesh| !mesh.indices.is_empty());
    out
}

#[cfg(test)]
mod tests;
