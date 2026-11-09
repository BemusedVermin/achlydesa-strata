//! `FigurePlugin`: figures as upright, camera-facing billboards (design §13.3, §13.8).
//!
//! The geometry, frame choice and fallback art live in [`strata_scene`]. This file keeps one
//! entity per figure on the board, turns it with the camera, and points its material at the
//! frame to show. The client fills [`FigureSprites`] from the packs' `assets.lua` and keeps
//! [`FigurePoses`] in step with the match.

use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_2;

use bevy::asset::RenderAssetUsages;
use bevy::math::{Affine2, UVec2};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use strata_kernel::{DefId, Hex, Rotation, Space, Sym};
use strata_model::FigureId;
use strata_scene::{
    IDLE, INSET, Orientation, PixelScale, Rgb, SpriteDef, billboard_rotation, direction, facing,
    foot_point, quad_corners, surface_point,
};

use crate::camera::{ClientBoard, IsoCamera};

/// How far a fallback's base floats above the surface; below the highlights.
const BASE_LIFT: f32 = 0.005;
/// Radius of a fallback's base on each cell: a hex's corner radius is one world unit, inset as
/// the figure's volume is.
const BASE_RADIUS: f32 = INSET;
/// Alpha below which a sprite pixel is cut out.
const CUTOFF: f32 = 0.5;

/// Which figure entry of which unit definition a sprite is for.
pub type LookKey = (DefId, Sym);

/// One figure kind's sprite, ready to draw.
#[derive(Clone, Debug)]
pub struct FigureSprite {
    /// Frame layout and animations.
    pub def: SpriteDef,
    /// The pixel scale it is drawn at.
    pub scale: PixelScale,
    /// The sprite sheet.
    pub sheet: Handle<Image>,
    /// The sheet's size in pixels.
    pub sheet_size: UVec2,
    /// `Some` for a generated fallback: it stands on a footprint-shaped base of this color, or of
    /// its seat's color when it is tinted by seat.
    pub base: Option<Rgb>,
    /// Whether to tint the sprite with its controller's seat color, for neutral fallbacks.
    pub tint_by_seat: bool,
    /// The figure's footprint cells around its anchor at rotation 0, in footprint order, which
    /// decide the way it faces (see [`strata_scene::facing`]).
    pub footprint: Vec<Hex>,
}

/// The sprite of every kind of figure; the client's assets plugin fills it in.
#[derive(Resource, Default)]
pub struct FigureSprites(pub BTreeMap<LookKey, FigureSprite>);

/// Where one figure stands and how it is turned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FigurePose {
    /// The figure.
    pub id: FigureId,
    /// Which sprite it uses.
    pub look: LookKey,
    /// Its anchor space.
    pub anchor: Space,
    /// Every space its footprint covers, anchor included.
    pub cells: Vec<Space>,
    /// Its facing.
    pub rotation: Rotation,
    /// The seat controlling it, for tinting.
    pub seat: u8,
}

/// The figures on the board; the client copies them from the match state.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct FigurePoses(pub Vec<FigurePose>);

/// Marks a figure's billboard (design §13.2).
#[derive(Component, Debug, PartialEq, Eq)]
pub struct FigureView {
    /// The figure it draws.
    pub id: FigureId,
}

/// The animation a figure is playing; playback sets it, and a finished one returns to `idle`.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct FigureAnimation {
    /// The animation's name; one the sprite lacks plays `idle`.
    pub name: Sym,
    /// Seconds since it started.
    pub elapsed: f32,
}

impl Default for FigureAnimation {
    fn default() -> Self {
        Self {
            name: Sym::new(IDLE),
            elapsed: 0.0,
        }
    }
}

/// Marks one cell of a fallback figure's base.
#[derive(Component, Debug, PartialEq, Eq)]
pub struct FigureBase {
    /// The figure it belongs to.
    pub id: FigureId,
}

/// What a billboard was spawned from, and the frame it shows.
#[derive(Component)]
struct Placed {
    pose: FigurePose,
    material: Handle<StandardMaterial>,
    shown: Option<(Sym, u8, u32)>,
}

/// Spawns, moves and removes figure billboards, and turns and animates them every frame.
pub struct FigurePlugin;

impl Plugin for FigurePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FigureSprites>()
            .init_resource::<FigurePoses>()
            .add_systems(Update, (sync_figures, animate_figures).chain());
    }
}

/// The billboard quad of a sprite, with its foot anchor at the origin.
fn quad_mesh(sprite: &FigureSprite) -> Mesh {
    let corners = quad_corners(&sprite.def, sprite.scale).map(|c| c.to_array());
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, corners.to_vec())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4])
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}

fn srgb(rgb: Rgb) -> Color {
    Color::srgb_u8(rgb[0], rgb[1], rgb[2])
}

#[derive(bevy::ecs::system::SystemParam)]
struct FigureAssets<'w, 's> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    quads: Local<'s, BTreeMap<LookKey, Handle<Mesh>>>,
    base: Local<'s, Option<Handle<Mesh>>>,
}

impl FigureAssets<'_, '_> {
    fn quad(&mut self, key: &LookKey, sprite: &FigureSprite) -> Handle<Mesh> {
        let meshes = &mut self.meshes;
        let quad = || meshes.add(quad_mesh(sprite));
        self.quads.entry(key.clone()).or_insert_with(quad).clone()
    }

    fn billboard_material(&mut self, sprite: &FigureSprite, seat: u8) -> Handle<StandardMaterial> {
        let tint = if sprite.tint_by_seat {
            srgb(strata_scene::seat_color(seat))
        } else {
            Color::WHITE
        };
        self.materials.add(StandardMaterial {
            base_color: tint,
            base_color_texture: Some(sprite.sheet.clone()),
            alpha_mode: AlphaMode::Mask(CUTOFF),
            unlit: true,
            cull_mode: None::<Face>,
            ..default()
        })
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct FigureInputs<'w> {
    poses: Res<'w, FigurePoses>,
    sprites: Res<'w, FigureSprites>,
    board: Option<Res<'w, ClientBoard>>,
}

/// Brings the billboards in line with [`FigurePoses`] when it, the sprites or the board change.
/// A figure keeps its entity, and so its animation, while it moves.
fn sync_figures(
    mut commands: Commands,
    inputs: FigureInputs,
    mut assets: FigureAssets,
    mut placed: Query<(Entity, &FigureView, &mut Placed, &mut Transform)>,
    bases: Query<Entity, With<FigureBase>>,
) {
    let FigureInputs {
        poses,
        sprites,
        board,
    } = inputs;
    let Some(board) = board else { return };
    if !poses.is_changed() && !sprites.is_changed() && !board.is_changed() {
        return;
    }
    let orientation = board.0.orientation;
    let rebuild_all = sprites.is_changed() || board.is_changed();
    if rebuild_all {
        // A look's sprite may have changed size, so its cached quad is stale.
        assets.quads.clear();
    }
    let wanted: BTreeMap<FigureId, &FigurePose> = poses.0.iter().map(|p| (p.id, p)).collect();
    let mut kept = BTreeMap::new();
    for (entity, view, mut placed, mut transform) in &mut placed {
        let pose = wanted.get(&view.id).copied();
        match pose {
            Some(pose) if !rebuild_all && same_look(pose, &placed.pose) => {
                transform.translation = foot(orientation, pose);
                placed.pose = pose.clone();
                kept.insert(view.id, entity);
            }
            _ => commands.entity(entity).despawn(),
        }
    }
    // Bases are cheap and follow every cell, so they are always rebuilt.
    for entity in &bases {
        commands.entity(entity).despawn();
    }
    for pose in &poses.0 {
        let Some(sprite) = sprites.0.get(&pose.look) else {
            continue;
        };
        if let Some(color) = sprite.base {
            let color = if sprite.tint_by_seat {
                strata_scene::seat_color(pose.seat)
            } else {
                color
            };
            spawn_base(&mut commands, &mut assets, orientation, pose, color);
        }
        if kept.contains_key(&pose.id) {
            continue;
        }
        let material = assets.billboard_material(sprite, pose.seat);
        commands.spawn((
            FigureView { id: pose.id },
            FigureAnimation::default(),
            Placed {
                pose: pose.clone(),
                material: material.clone(),
                shown: None,
            },
            Mesh3d(assets.quad(&pose.look, sprite)),
            MeshMaterial3d(material),
            Transform::from_translation(foot(orientation, pose)),
        ));
    }
}

/// Whether a billboard drawn for `old` can be moved to `new` rather than drawn afresh.
fn same_look(new: &FigurePose, old: &FigurePose) -> bool {
    new.look == old.look && new.seat == old.seat
}

fn foot(orientation: Orientation, pose: &FigurePose) -> Vec3 {
    let cells: Vec<_> = pose.cells.iter().map(|s| s.hex()).collect();
    foot_point(orientation, pose.anchor, &cells)
}

/// One flat hex in a darker shade of the figure's color under each cell it covers.
fn spawn_base(
    commands: &mut Commands,
    assets: &mut FigureAssets,
    orientation: Orientation,
    pose: &FigurePose,
    color: Rgb,
) {
    let meshes = &mut assets.meshes;
    let mesh = assets
        .base
        .get_or_insert_with(|| meshes.add(RegularPolygon::new(BASE_RADIUS, 6)))
        .clone();
    let material = assets.materials.add(StandardMaterial {
        base_color: srgb(color.map(|c| c / 2)),
        unlit: true,
        ..default()
    });
    // The polygon has a corner toward +Y; laid flat, that corner points along the depth axis,
    // which is the pointy layout. Flat hexes are a twelfth of a turn round.
    let yaw = orientation.first_direction_angle();
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-FRAC_PI_2);
    for cell in &pose.cells {
        let at = surface_point(orientation, cell.hex(), cell.z) + Vec3::Y * BASE_LIFT;
        commands.spawn((
            FigureBase { id: pose.id },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at).with_rotation(rotation),
        ));
    }
}

/// Turns every billboard toward the camera and shows the frame for its facing and animation.
fn animate_figures(
    time: Res<Time>,
    camera: Res<IsoCamera>,
    sprites: Res<FigureSprites>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut figures: Query<(&mut Placed, &mut FigureAnimation, &mut Transform)>,
) {
    let view = &camera.0;
    let (rotation, step) = (billboard_rotation(view.yaw), view.yaw_step());
    for (mut placed, mut animation, mut transform) in &mut figures {
        transform.rotation = rotation;
        let Some(sprite) = sprites.0.get(&placed.pose.look) else {
            continue;
        };
        animation.elapsed += time.delta_secs();
        let mut pose = sprite.def.pose(animation.name.as_str(), animation.elapsed);
        if pose.finished {
            *animation = FigureAnimation::default();
            pose = sprite.def.pose(IDLE, 0.0);
        }
        let toward = direction(facing(&sprite.footprint, placed.pose.rotation), step);
        let shown = Some((pose.animation.clone(), toward, pose.frame));
        if placed.shown == shown {
            continue;
        }
        let (min, max) = sprite.def.uv_rect(sprite.sheet_size, &pose, toward);
        if let Some(mut material) = materials.get_mut(&placed.material) {
            material.uv_transform = Affine2::from_scale_angle_translation(max - min, 0.0, min);
        }
        placed.shown = shown;
    }
}

#[cfg(test)]
mod tests;
