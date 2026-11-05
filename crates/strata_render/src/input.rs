//! `InputPlugin`: board clicks and the highlighted spaces (design §13.3, §13.5).
//!
//! What a click means is decided by the client, against the legal commands or the pending
//! decision. This plugin only reports which space was clicked and outlines the spaces it is told
//! to.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_6};

use bevy::prelude::*;
use strata_kernel::Space;
use strata_scene::{Orientation, surface_point};

use crate::camera::{ClientBoard, HoveredSpace};

/// How far a highlight floats above the surface, so it does not flicker against it.
const LIFT: f32 = 0.01;
/// Radius of a highlight, a little inside the hex's corners.
const RADIUS: f32 = 0.85;

/// A left click on the board, at the space under the cursor.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpaceClicked(pub Space);

/// The spaces to mark on the board, such as the legal choices of a decision.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct BoardHighlights(pub Vec<Space>);

/// Marks the entities that draw [`BoardHighlights`].
#[derive(Component)]
struct HighlightMark;

/// The mesh and material every highlight shares.
#[derive(Resource)]
struct HighlightLook {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

/// Reports clicks on the board and draws the highlighted spaces.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoardHighlights>()
            .add_message::<SpaceClicked>()
            .add_systems(Startup, make_look)
            .add_systems(Update, (click, draw_highlights));
    }
}

fn make_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material = StandardMaterial {
        base_color: Color::srgba(1.0, 0.85, 0.2, 0.55),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    };
    commands.insert_resource(HighlightLook {
        mesh: meshes.add(RegularPolygon::new(RADIUS, 6)),
        material: materials.add(material),
    });
}

/// Sends a [`SpaceClicked`] when the left button goes down over the board.
fn click(
    buttons: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredSpace>,
    mut clicked: MessageWriter<SpaceClicked>,
) {
    if buttons.just_pressed(MouseButton::Left)
        && let Some(space) = hovered.0
    {
        clicked.write(SpaceClicked(space));
    }
}

/// Replaces the highlight entities whenever the highlighted spaces or the board change. With no
/// board, nothing is drawn.
fn draw_highlights(
    mut commands: Commands,
    highlights: Res<BoardHighlights>,
    look: Option<Res<HighlightLook>>,
    board: Option<Res<ClientBoard>>,
    drawn: Query<Entity, With<HighlightMark>>,
) {
    let Some(look) = look else { return };
    let board_changed = board.as_ref().is_some_and(Res::is_changed);
    if !highlights.is_changed() && !board_changed {
        return;
    }
    for entity in &drawn {
        commands.entity(entity).despawn();
    }
    let Some(board) = board else { return };
    let orientation = board.0.orientation;
    // The polygon mesh lies in the XY plane with a corner toward +Y; laid flat, that corner
    // points along the depth axis, which is the pointy layout.
    let yaw = match orientation {
        Orientation::Pointy => 0.0,
        Orientation::Flat => FRAC_PI_6,
    };
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-FRAC_PI_2);
    for space in &highlights.0 {
        let at = surface_point(orientation, space.hex(), space.z) + Vec3::Y * LIFT;
        commands.spawn((
            HighlightMark,
            Mesh3d(look.mesh.clone()),
            MeshMaterial3d(look.material.clone()),
            Transform::from_translation(at).with_rotation(rotation),
        ));
    }
}
