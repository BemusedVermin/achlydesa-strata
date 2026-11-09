use std::collections::BTreeMap;
use std::f32::consts::{FRAC_PI_3, TAU};

use glam::{UVec2, Vec3};
use strata_kernel::{Hex, Rotation, Space, Sym};

use super::*;
use crate::layout::{HEX_WIDTH, LEVEL_HEIGHT, Orientation, surface_point};
use crate::view::IsoView;

/// A 48x72 sprite with its foot at (24, 68), and `idle` plus `move`.
pub(crate) fn soldier() -> SpriteDef {
    let animations = BTreeMap::from([
        (Sym::new("move"), Animation { frames: 6, fps: 10 }),
        (Sym::new(IDLE), Animation { frames: 4, fps: 6 }),
    ]);
    SpriteDef {
        key: Sym::new("sprite.test_soldier"),
        frame: UVec2::new(48, 72),
        anchor: UVec2::new(24, 68),
        animations,
    }
}

fn view_at(step: u32) -> IsoView {
    let mut view = IsoView::default();
    view.turn(step as i32);
    view.yaw = view.step as f32 * FRAC_PI_3;
    view
}

#[test]
fn the_direction_is_the_facing_turned_by_the_camera_step() {
    let facing = |k| Rotation::from_steps(k);
    assert_eq!(direction(facing(0), 0), 0);
    assert_eq!(direction(facing(2), 0), 2);
    assert_eq!(direction(facing(2), 3), 5);
    assert_eq!(direction(facing(5), 2), 1);
    // Turning the figure one way and the camera the other shows the same frame.
    for f in 0..6 {
        for s in 0..6u32 {
            let back = facing(f + 1);
            assert_eq!(direction(facing(f), s), direction(back, (s + 5) % 6));
        }
    }
}

/// The angle on screen at which a figure facing `facing` points, seen from yaw step `step`,
/// measured from screen right toward the viewer.
fn screen_angle(orientation: Orientation, facing: u8, step: u32) -> f32 {
    let view = view_at(step);
    let angle = orientation.first_direction_angle() + f32::from(facing) * FRAC_PI_3;
    let world = Vec3::new(angle.cos(), 0.0, angle.sin());
    let toward_viewer = -Vec3::new(view.forward().x, 0.0, view.forward().z).normalize();
    world
        .dot(toward_viewer)
        .atan2(world.dot(view.right()))
        .rem_euclid(TAU)
}

#[test]
fn the_frame_shown_matches_the_facing_seen_on_screen() {
    for orientation in [Orientation::Pointy, Orientation::Flat] {
        for f in 0..6u8 {
            for s in 0..6 {
                let d = direction(Rotation::from_steps(i32::from(f)), s);
                let seen = screen_angle(orientation, f, s);
                let drawn = screen_angle(orientation, d, 0);
                let gap = (seen - drawn).abs();
                assert!(gap < 1e-3 || (TAU - gap) < 1e-3, "facing {f}, step {s}");
            }
        }
    }
}

#[test]
fn the_camera_step_used_for_frames_is_the_nearest_to_the_drawn_yaw() {
    let mut view = IsoView {
        yaw: FRAC_PI_3 * 0.4,
        ..IsoView::default()
    };
    assert_eq!(view.yaw_step(), 0);
    view.yaw = FRAC_PI_3 * 0.6;
    assert_eq!(view.yaw_step(), 1);
    view.yaw = -FRAC_PI_3;
    assert_eq!(view.yaw_step(), 5);
}

#[test]
fn pixels_scale_to_levels_vertically_and_hexes_horizontally() {
    let default = PixelScale::default().world_per_pixel();
    assert!((default.y * 12.0 - LEVEL_HEIGHT).abs() < 1e-6);
    assert!((default.x * 64.0 - HEX_WIDTH).abs() < 1e-6);
    // The default proportions give square pixels.
    assert!((default.x - default.y).abs() < 1e-3);
    let fine = PixelScale {
        pixels_per_level: 24,
        hex_width_px: 128,
    }
    .world_per_pixel();
    assert!((fine.y * 24.0 - LEVEL_HEIGHT).abs() < 1e-6);
    assert!((fine.x * 128.0 - HEX_WIDTH).abs() < 1e-6);
    let scale = PixelScale {
        pixels_per_level: 16,
        hex_width_px: 64,
    };
    assert_eq!(scale.sub_levels_to_px(12), 16);
    assert_eq!(scale.sub_levels_to_px(20), 27, "26.7 rounds to 27");
    assert!((scale.px_to_sub_levels(16) - 12.0).abs() < 1e-6);
}

#[test]
fn the_quad_hangs_from_the_foot_anchor() {
    let def = soldier();
    let scale = PixelScale::default();
    let px = scale.world_per_pixel();
    let [bl, br, tr, tl] = quad_corners(&def, scale);
    assert!((bl.x + 24.0 * px.x).abs() < 1e-6 && (br.x - 24.0 * px.x).abs() < 1e-6);
    assert!(
        (bl.y + 4.0 * px.y).abs() < 1e-6,
        "four pixels below the foot"
    );
    assert!(
        (tl.y - 68.0 * px.y).abs() < 1e-6,
        "68 pixels above the foot"
    );
    assert_eq!((tr.y, tr.x), (tl.y, br.x));
    assert!([bl, br, tr, tl].iter().all(|c| c.z == 0.0));
}

#[test]
fn billboards_face_the_camera_and_never_tilt() {
    for step in 0..6 {
        let view = view_at(step);
        let rotation = billboard_rotation(view.yaw);
        let normal = rotation * Vec3::Z;
        let to_eye = view.eye() - view.focus;
        let to_eye = Vec3::new(to_eye.x, 0.0, to_eye.z).normalize();
        assert!((normal - to_eye).length() < 1e-4, "step {step}");
        assert!(
            (rotation * Vec3::Y - Vec3::Y).length() < 1e-6,
            "upright at step {step}"
        );
        assert!(
            (rotation * Vec3::X - view.right()).length() < 1e-4,
            "width along screen right"
        );
    }
}

#[test]
fn a_figure_stands_over_the_middle_of_its_cells_on_its_anchor_surface() {
    let anchor = Space::at(Hex::new(2, -1), 3);
    for orientation in [Orientation::Pointy, Orientation::Flat] {
        let one = foot_point(orientation, anchor, &[anchor.hex()]);
        assert!((one - surface_point(orientation, anchor.hex(), 3)).length() < 1e-6);
        assert_eq!(foot_point(orientation, anchor, &[]), one);
        let pair = [anchor.hex(), anchor.hex().neighbor(0)];
        let two = foot_point(orientation, anchor, &pair);
        let other = surface_point(orientation, pair[1], 3);
        assert!((two - (one + other) / 2.0).length() < 1e-5);
    }
}

#[test]
fn the_sheet_stacks_six_direction_rows_per_animation_with_idle_first() {
    let mut def = soldier();
    def.animations
        .insert(Sym::new("dance"), Animation { frames: 2, fps: 4 });
    def.animations
        .insert(Sym::new("attack"), Animation { frames: 5, fps: 12 });
    let order: Vec<_> = def.sheet_order().iter().map(|s| s.as_str()).collect();
    assert_eq!(order, [IDLE, "move", "attack", "dance"]);
    assert_eq!(def.sheet_size(), UVec2::new(6 * 48, 24 * 72));
    assert_eq!(def.cell(IDLE, 0, 0), Some(UVec2::ZERO));
    assert_eq!(def.cell(IDLE, 5, 3), Some(UVec2::new(3 * 48, 5 * 72)));
    assert_eq!(def.cell("move", 2, 1), Some(UVec2::new(48, 8 * 72)));
    assert_eq!(def.cell("attack", 0, 0), Some(UVec2::new(0, 12 * 72)));
    assert_eq!(def.cell("hit", 0, 0), None);
    let pose = def.pose("move", 0.15);
    let (min, max) = def.uv_rect(def.sheet_size(), &pose, 2);
    assert!((min.x - 1.0 / 6.0).abs() < 1e-6 && (min.y - 8.0 / 24.0).abs() < 1e-6);
    assert!((max.x - 2.0 / 6.0).abs() < 1e-6 && (max.y - 9.0 / 24.0).abs() < 1e-6);
}

#[test]
fn idle_loops_and_other_animations_play_once() {
    let def = soldier();
    let at = |name: &str, t: f32| def.pose(name, t);
    assert_eq!(at(IDLE, 0.0).frame, 0);
    assert_eq!(at(IDLE, 0.5).frame, 3);
    assert_eq!(
        at(IDLE, 0.7).frame,
        0,
        "4 frames at 6 fps wrap after 0.67 s"
    );
    assert!(!at(IDLE, 100.0).finished);
    assert_eq!(
        (at("move", 0.35).frame, at("move", 0.35).finished),
        (3, false)
    );
    let done = at("move", 0.65);
    assert_eq!(
        (done.frame, done.finished),
        (5, true),
        "holds the last frame"
    );
}

#[test]
fn missing_animations_fall_back_to_idle() {
    let def = soldier();
    for name in ["attack", "hit", "death", "anything"] {
        let pose = def.pose(name, 0.5);
        assert_eq!(pose.animation.as_str(), IDLE, "{name}");
        assert_eq!(pose.frame, 3);
        assert!(!pose.finished);
    }
    let empty = SpriteDef {
        animations: BTreeMap::new(),
        ..def
    };
    assert_eq!(empty.pose("move", 1.0).frame, 0);
}
