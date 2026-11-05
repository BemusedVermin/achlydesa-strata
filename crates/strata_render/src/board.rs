//! `BoardPlugin`: draws the compiled board as chunked, flat-shaded hex-prism meshes (design §13.3).
//!
//! The face generation lives in [`strata_scene`]; this file turns it into Bevy meshes and keeps
//! them current. Textures from `assets.lua` arrive with the assets plugin; until a terrain has
//! one it gets a tint derived from its id (design §13.4).

use std::collections::{BTreeMap, BTreeSet};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use strata_kernel::{Hex, Sym};
use strata_scene::{
    ChunkKey, Material, MeshData, MeshOptions, chunk_meshes, chunk_of, chunks, feature_chunks,
};

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

    /// Marks these chunks for rebuilding.
    pub fn mark_chunks(&mut self, chunks: impl IntoIterator<Item = ChunkKey>) {
        self.chunks.extend(chunks);
    }

    /// Marks every chunk for rebuilding.
    pub fn mark_all(&mut self) {
        self.all = true;
    }
}

/// Placed features removed during play; the client copies them from the match state.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct RemovedFeatures(pub BTreeSet<u32>);

/// The name of each placed feature, indexed like the board's feature spans.
#[derive(Resource, Default, Debug)]
pub struct FeatureNames(pub Vec<Sym>);

/// Marks a spawned terrain chunk mesh.
#[derive(Component)]
pub struct TerrainChunk {
    /// The chunk it draws.
    pub key: ChunkKey,
    /// The material it draws.
    pub material: Material,
}

/// Builds and refreshes the terrain meshes whenever [`ClientBoard`], [`LevelSlice`] or
/// [`DirtyChunks`] change.
pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelSlice>()
            .init_resource::<RemovedFeatures>()
            .init_resource::<FeatureNames>()
            .init_resource::<DirtyChunks>()
            .add_systems(Update, (detect_changes, rebuild).chain());
    }
}

/// Marks everything stale when the board or the slice changes, and only the chunks holding a
/// feature when that feature is removed (or put back).
fn detect_changes(
    board: Option<Res<ClientBoard>>,
    slice: Res<LevelSlice>,
    removed: Res<RemovedFeatures>,
    mut known: Local<BTreeSet<u32>>,
    mut dirty: ResMut<DirtyChunks>,
) {
    let Some(board) = board else { return };
    if board.is_changed() || slice.is_changed() {
        dirty.mark_all();
    }
    if removed.is_changed() {
        let changed: BTreeSet<u32> = known.symmetric_difference(&removed.0).copied().collect();
        dirty.mark_chunks(feature_chunks(&board.0, &changed));
        known.clone_from(&removed.0);
    }
}

/// A stable, distinct tint for a material without a texture.
fn tint(material: Material, names: &FeatureNames) -> Color {
    match material {
        Material::Terrain(id) => Color::hsl((id.0.wrapping_mul(137) % 360) as f32, 0.45, 0.55),
        Material::Feature(index) => {
            let name = names.0.get(index as usize).map_or("", |n| n.as_str());
            let hash = name
                .bytes()
                .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b.into()));
            Color::hsl((hash % 360) as f32, 0.3, 0.4)
        }
    }
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
struct BoardInputs<'w> {
    slice: Res<'w, LevelSlice>,
    removed: Res<'w, RemovedFeatures>,
    names: Res<'w, FeatureNames>,
}

#[derive(bevy::ecs::system::SystemParam)]
struct BoardAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

fn rebuild(
    mut commands: Commands,
    board: Option<Res<ClientBoard>>,
    inputs: BoardInputs,
    mut dirty: ResMut<DirtyChunks>,
    existing: Query<(Entity, &TerrainChunk)>,
    mut assets: BoardAssets,
    mut cache: Local<BTreeMap<Material, Handle<StandardMaterial>>>,
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
        let options = MeshOptions {
            hide_from: inputs.slice.hide_from,
            removed: Some(&inputs.removed.0),
        };
        for (material, data) in chunk_meshes(&board.0, key, options) {
            let handle = cache
                .entry(material)
                .or_insert_with(|| {
                    assets.materials.add(StandardMaterial {
                        base_color: tint(material, &inputs.names),
                        perceptual_roughness: 1.0,
                        ..default()
                    })
                })
                .clone();
            commands.spawn((
                TerrainChunk { key, material },
                Mesh3d(assets.meshes.add(to_bevy(data))),
                MeshMaterial3d(handle),
            ));
        }
    }
}

#[cfg(test)]
mod tests;
