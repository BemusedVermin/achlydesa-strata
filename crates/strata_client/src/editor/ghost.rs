//! Draws the placement ghost: an outline on the top face each placed hex would have, tinted by
//! whether it fits.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_6};

use bevy::prelude::*;
use strata_scene::{Orientation, surface_point};

use super::{Editing, EditorFeedback};

/// How far the ghost floats above the face, so it does not flicker against terrain there.
const LIFT: f32 = 0.02;
/// Radius of a ghost hex, a little inside the hex's corners.
const RADIUS: f32 = 0.9;

/// Marks the entities that draw the ghost.
#[derive(Component)]
pub(super) struct GhostMark;

/// The mesh every ghost hex shares, and the materials for hexes that fit and hexes that do not.
#[derive(Resource)]
pub(super) struct GhostLook {
    mesh: Handle<Mesh>,
    fits: Handle<StandardMaterial>,
    refused: Handle<StandardMaterial>,
}

pub(super) fn make_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut tint = |color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })
    };
    commands.insert_resource(GhostLook {
        mesh: meshes.add(RegularPolygon::new(RADIUS, 6)),
        fits: tint(Color::srgba(0.3, 0.9, 1.0, 0.5)),
        refused: tint(Color::srgba(1.0, 0.25, 0.2, 0.55)),
    });
}

/// Replaces the ghost's entities whenever the preview changes.
pub(super) fn draw(
    mut commands: Commands,
    feedback: Res<EditorFeedback>,
    editing: Res<Editing>,
    look: Option<Res<GhostLook>>,
    drawn: Query<Entity, With<GhostMark>>,
) {
    let Some(look) = look else { return };
    if !feedback.is_changed() {
        return;
    }
    for entity in &drawn {
        commands.entity(entity).despawn();
    }
    let orientation = editing.orientation();
    // As for the board's highlights: the polygon lies in the XY plane with a corner toward +Y,
    // and laid flat that corner points along the depth axis, which is the pointy layout.
    let yaw = match orientation {
        Orientation::Pointy => 0.0,
        Orientation::Flat => FRAC_PI_6,
    };
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-FRAC_PI_2);
    for mark in &feedback.preview.marks {
        let space = mark.space;
        let at = surface_point(orientation, space.hex(), space.z) + Vec3::Y * LIFT;
        let material = if mark.ok { &look.fits } else { &look.refused };
        commands.spawn((
            GhostMark,
            Mesh3d(look.mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at).with_rotation(rotation),
        ));
    }
}
