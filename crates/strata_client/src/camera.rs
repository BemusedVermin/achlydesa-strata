//! `CameraPlugin`: the orthographic isometric camera and cursor picking (design §13.3, §13.8).
//!
//! The math lives in [`crate::scene`]; this file only wires it to Bevy.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use strata_kernel::Space;

use crate::scene::{IsoView, SceneBoard, pick};

/// Pan speed in world units per second at zoom 1.
const PAN_SPEED: f32 = 8.0;

/// The player's camera.
#[derive(Resource, Default)]
pub struct IsoCamera(pub IsoView);

/// The board the client picks against; the board plugin fills it in.
#[derive(Resource)]
pub struct ClientBoard(pub SceneBoard);

/// The space under the cursor, if any.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct HoveredSpace(pub Option<Space>);

/// Marks the isometric camera entity.
#[derive(Component)]
pub struct IsoCameraMarker;

/// Spawns the camera, reads the keys and wheel, and keeps [`HoveredSpace`] current.
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IsoCamera>()
            .init_resource::<HoveredSpace>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (control_camera, apply_camera, hover).chain());
    }
}

fn transform(view: &IsoView) -> Transform {
    Transform::from_translation(view.eye()).looking_at(view.focus, Vec3::Y)
}

fn spawn_camera(mut commands: Commands, camera: Res<IsoCamera>) {
    commands.spawn((
        IsoCameraMarker,
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: camera.0.scale(),
            ..OrthographicProjection::default_3d()
        }),
        transform(&camera.0),
    ));
}

/// Q and E turn, the wheel and `-`/`=` zoom, and WASD or the arrows pan.
fn control_camera(
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    time: Res<Time>,
    mut camera: ResMut<IsoCamera>,
) {
    let view = &mut camera.0;
    let pressed = |codes: &[KeyCode]| codes.iter().any(|k| keys.pressed(*k));
    if keys.just_pressed(KeyCode::KeyQ) {
        view.turn(-1);
    }
    if keys.just_pressed(KeyCode::KeyE) {
        view.turn(1);
    }
    if keys.just_pressed(KeyCode::Equal) {
        view.zoom_by(1);
    }
    if keys.just_pressed(KeyCode::Minus) {
        view.zoom_by(-1);
    }
    for event in wheel.read() {
        view.zoom_by(event.y.signum() as i32);
    }
    let axis =
        |plus: &[KeyCode], minus: &[KeyCode]| f32::from(pressed(plus)) - f32::from(pressed(minus));
    let right = axis(
        &[KeyCode::KeyD, KeyCode::ArrowRight],
        &[KeyCode::KeyA, KeyCode::ArrowLeft],
    );
    let forward = axis(
        &[KeyCode::KeyW, KeyCode::ArrowUp],
        &[KeyCode::KeyS, KeyCode::ArrowDown],
    );
    let speed = PAN_SPEED / view.zoom as f32 * time.delta_secs();
    view.pan(right * speed, forward * speed);
    view.ease(time.delta_secs());
}

/// Copies the camera resource onto the camera entity.
fn apply_camera(
    camera: Res<IsoCamera>,
    mut entity: Single<(&mut Transform, &mut Projection), With<IsoCameraMarker>>,
) {
    *entity.0 = transform(&camera.0);
    if let Projection::Orthographic(ortho) = &mut *entity.1 {
        ortho.scale = camera.0.scale();
    }
}

/// Casts the cursor ray into the board.
fn hover(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<IsoCameraMarker>>,
    board: Option<Res<ClientBoard>>,
    mut hovered: ResMut<HoveredSpace>,
) {
    let space = (|| {
        let board = board?;
        let cursor = window.cursor_position()?;
        let ray = camera.0.viewport_to_world(camera.1, cursor).ok()?;
        pick(&board.0, ray.origin, *ray.direction)
    })();
    if hovered.0 != space {
        hovered.0 = space;
    }
}
