//! `BoardPlugin`: draws the compiled board as chunked, flat-shaded hex-prism meshes (design §13.3).
//!
//! The face generation lives in [`strata_scene`]; this file turns it into Bevy meshes and keeps
//! them current. Textures from `assets.lua` arrive with the assets plugin; until a terrain has
//! one it gets a tint derived from its id (design §13.4).

use std::collections::{BTreeMap, BTreeSet};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use strata_kernel::{DefId, Hex};
use strata_scene::{ChunkKey, MeshData, chunk_meshes, chunk_of, chunks};

use crate::camera::ClientBoard;

/// Which levels are drawn; the editor's level-slicing hook.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct LevelSlice {
    /// Levels at or above this are hidden; `None` shows everything.
    pub hide_from: Option<u32>,
}

/// The chunks whose meshes are stale. Insert hexes here when terrain under them changes.
#[derive(Resource, Default, Debug)]
pub struct DirtyChunks {
    chunks: BTreeSet<ChunkKey>,
    all: bool,
}

impl DirtyChunks {
    /// Marks the chunk holding `hex` for rebuilding.
    pub fn mark(&mut self, hex: Hex) {
        self.chunks.insert(chunk_of(hex));
    }

    /// Marks every chunk for rebuilding.
    pub fn mark_all(&mut self) {
        self.all = true;
    }
}

/// Marks a spawned terrain chunk mesh.
#[derive(Component)]
pub struct TerrainChunk {
    /// The chunk it draws.
    pub key: ChunkKey,
    /// The terrain material it draws.
    pub terrain: DefId,
}

/// Builds and refreshes the terrain meshes whenever [`ClientBoard`], [`LevelSlice`] or
/// [`DirtyChunks`] change.
pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelSlice>()
            .init_resource::<DirtyChunks>()
            .add_systems(Update, (detect_changes, rebuild).chain());
    }
}

fn detect_changes(
    board: Option<Res<ClientBoard>>,
    slice: Res<LevelSlice>,
    mut dirty: ResMut<DirtyChunks>,
) {
    let board_new = board.is_some_and(|b| b.is_changed());
    if board_new || slice.is_changed() {
        dirty.mark_all();
    }
}

/// A stable, distinct tint for a terrain without a texture.
fn tint(terrain: DefId) -> Color {
    let hue = (terrain.0.wrapping_mul(137) % 360) as f32;
    Color::hsl(hue, 0.45, 0.55)
}

fn to_bevy(data: MeshData) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs)
    .with_inserted_indices(Indices::U32(data.indices))
}

#[derive(bevy::ecs::system::SystemParam)]
struct BoardAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

fn rebuild(
    mut commands: Commands,
    board: Option<Res<ClientBoard>>,
    slice: Res<LevelSlice>,
    mut dirty: ResMut<DirtyChunks>,
    existing: Query<(Entity, &TerrainChunk)>,
    mut assets: BoardAssets,
    mut cache: Local<BTreeMap<DefId, Handle<StandardMaterial>>>,
) {
    let Some(board) = board else { return };
    if !dirty.all && dirty.chunks.is_empty() {
        return;
    }
    let todo = if dirty.all {
        chunks(&board.0)
    } else {
        std::mem::take(&mut dirty.chunks)
    };
    let stale_all = std::mem::take(&mut dirty.all);
    for (entity, chunk) in &existing {
        if stale_all || todo.contains(&chunk.key) {
            commands.entity(entity).despawn();
        }
    }
    for key in todo {
        for (terrain, data) in chunk_meshes(&board.0, key, slice.hide_from) {
            let material = cache
                .entry(terrain)
                .or_insert_with(|| {
                    assets.materials.add(StandardMaterial {
                        base_color: tint(terrain),
                        perceptual_roughness: 1.0,
                        ..default()
                    })
                })
                .clone();
            commands.spawn((
                TerrainChunk { key, terrain },
                Mesh3d(assets.meshes.add(to_bevy(data))),
                MeshMaterial3d(material),
            ));
        }
    }
}
