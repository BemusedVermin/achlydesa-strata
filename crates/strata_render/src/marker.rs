//! Flat hex markers laid on a space's top face, shared by the board highlights and the
//! editor's placement ghost.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_6};

use bevy::prelude::*;
use strata_kernel::Space;
use strata_scene::{Orientation, surface_point};

/// The flat hexagon of radius `radius` (1 reaches the hex's corners) every marker shares.
pub fn marker_mesh(radius: f32) -> Mesh {
    RegularPolygon::new(radius, 6).into()
}

/// An unlit, see-through material of `color` for markers.
pub fn marker_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    }
}

/// Where a marker on the top face of `space` goes, `lift` above it so it does not flicker
/// against the surface.
pub fn marker_transform(orientation: Orientation, space: Space, lift: f32) -> Transform {
    // The polygon mesh lies in the XY plane with a corner toward +Y; laid flat, that corner
    // points along the depth axis, which is the pointy layout.
    let yaw = match orientation {
        Orientation::Pointy => 0.0,
        Orientation::Flat => FRAC_PI_6,
    };
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-FRAC_PI_2);
    let at = surface_point(orientation, space.hex(), space.z) + Vec3::Y * lift;
    Transform::from_translation(at).with_rotation(rotation)
}
