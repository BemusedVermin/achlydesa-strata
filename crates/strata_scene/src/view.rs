//! The isometric view: elevation, yaw steps, integer zoom and pan.

use std::f32::consts::{PI, TAU};

use glam::Vec3;

/// Yaw turned by one step.
const YAW_STEP: f32 = PI / 3.0;
/// Pixels per world unit at zoom 1: a hex corner is about 37 px for a 64 px hex width.
const BASE_PIXELS_PER_UNIT: f32 = 37.0;
/// Largest integer zoom step.
pub const MAX_ZOOM: u32 = 6;
/// Fraction of the remaining yaw covered per second while turning.
const TURN_RATE: f32 = 10.0;
/// How far the eye sits from the focus; large enough to clear any terrain.
const DISTANCE: f32 = 200.0;

/// Where the camera looks from.
#[derive(Clone, Debug)]
pub struct IsoView {
    /// Elevation above the ground plane, in radians.
    pub elevation: f32,
    /// The yaw step the player asked for, 0 to 5.
    pub step: u32,
    /// The yaw drawn this frame, easing toward the step.
    pub yaw: f32,
    /// Integer zoom step, 1 to [`MAX_ZOOM`].
    pub zoom: u32,
    /// The world point at the center of the view.
    pub focus: Vec3,
}

impl Default for IsoView {
    fn default() -> Self {
        Self {
            elevation: 30f32.to_radians(),
            step: 0,
            yaw: 0.0,
            zoom: 2,
            focus: Vec3::ZERO,
        }
    }
}

impl IsoView {
    /// Turns by `by` steps; positive turns clockwise seen from above.
    pub fn turn(&mut self, by: i32) {
        self.step = (self.step as i32 + by).rem_euclid(6) as u32;
    }

    /// Changes the zoom step by `by`, staying within range.
    pub fn zoom_by(&mut self, by: i32) {
        self.zoom = (self.zoom as i32 + by).clamp(1, MAX_ZOOM as i32) as u32;
    }

    /// The yaw step nearest the drawn yaw, 0 to 5; what billboards pick their frames by.
    pub fn yaw_step(&self) -> u32 {
        ((self.yaw / YAW_STEP).round() as i32).rem_euclid(6) as u32
    }

    /// The unit vector pointing to the screen's right, along the ground.
    pub fn right(&self) -> Vec3 {
        let (sin_y, cos_y) = self.yaw.sin_cos();
        Vec3::new(cos_y, 0.0, -sin_y)
    }

    /// Eases the drawn yaw toward the chosen step along the shorter way round.
    pub fn ease(&mut self, dt: f32) {
        let target = self.step as f32 * YAW_STEP;
        let delta = (target - self.yaw + PI).rem_euclid(TAU) - PI;
        self.yaw = if delta.abs() < 1e-3 {
            target
        } else {
            self.yaw + delta * (TURN_RATE * dt).min(1.0)
        };
    }

    /// Moves the focus by `right` and `forward` units along the ground, as seen at the current yaw.
    pub fn pan(&mut self, right: f32, forward: f32) {
        let (sin_y, cos_y) = self.yaw.sin_cos();
        self.focus += self.right() * right - Vec3::new(sin_y, 0.0, cos_y) * forward;
    }

    /// The eye position, `DISTANCE` away from the focus.
    pub fn eye(&self) -> Vec3 {
        self.focus - self.forward() * DISTANCE
    }

    /// The unit direction the camera looks in.
    pub fn forward(&self) -> Vec3 {
        let (sin_e, cos_e) = self.elevation.sin_cos();
        let (sin_y, cos_y) = self.yaw.sin_cos();
        -Vec3::new(cos_e * sin_y, sin_e, cos_e * cos_y)
    }

    /// World units per pixel; integer zoom keeps texels whole.
    pub fn scale(&self) -> f32 {
        1.0 / (BASE_PIXELS_PER_UNIT * self.zoom as f32)
    }
}
