//! Figure sprites: sheet layout, directional frames, animation and pixel scale (design §13.8).
//!
//! # Sheet layout
//!
//! A sheet is a grid of equal frames. Each animation takes six consecutive rows, one per
//! direction `0..6`, with its frames left to right. Animations are stacked top to bottom in the
//! order `idle`, `move`, `attack`, `hit`, `death`, then any others by name; only the animations a
//! sprite declares take rows.
//!
//! # Directions
//!
//! Direction `d` shows the figure facing hex direction `d` while the camera is at yaw step 0.
//! A figure's facing comes from its rotation and footprint ([`facing`]): a one-cell figure faces
//! its rotation, a larger one faces away from its rear. Turning the camera one step turns the
//! whole picture one step, so a figure facing `f` seen from camera step `s` shows direction
//! `(f + s) mod 6`; see [`direction`].

mod billboard;
mod catalog;
mod facing;
mod fallback;

use std::collections::BTreeMap;

use glam::{UVec2, Vec2};
use strata_kernel::lattice::{INSET_DEN, INSET_NUM};
use strata_kernel::{Rotation, Sym};

use super::layout::{HEX_WIDTH, LEVEL_HEIGHT};

pub use self::billboard::{billboard_rotation, foot_point, quad_corners};
pub use self::catalog::{
    ASSETS_DIR, AssetError, MAX_FPS, MAX_FRAME_PX, MAX_FRAMES, MAX_HEX_WIDTH_PX,
    MAX_PIXELS_PER_LEVEL, MAX_SHEET_PX, SpriteCatalog, SpriteEntry,
};
pub use self::facing::facing;
pub use self::fallback::{FallbackSprite, Rgb, fallback_sprite, parse_color, seat_color};

/// The animation every sprite must have, and the one the others fall back to.
pub const IDLE: &str = "idle";
/// The animations with a fixed place in the sheet, in sheet order.
pub const ANIMATIONS: [&str; 5] = [IDLE, "move", "attack", "hit", "death"];
/// Sub-levels per level, the kernel's unit of LOS heights.
pub(crate) const SUB_LEVELS: u32 = strata_kernel::SUB_LEVELS as u32;
/// How much of each hex a figure's volume covers across: the kernel's 80% inset (design §7.6).
pub const INSET: f32 = INSET_NUM as f32 / INSET_DEN as f32;

/// How a pack's pixel art maps onto the board (design §13.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelScale {
    /// Sprite pixels per level of height.
    pub pixels_per_level: u32,
    /// Sprite pixels across a hex's flats.
    pub hex_width_px: u32,
}

impl Default for PixelScale {
    /// 12 pixels per level against a 64-pixel hex, the proportions the terrain is built at.
    fn default() -> Self {
        Self {
            pixels_per_level: 12,
            hex_width_px: 64,
        }
    }
}

impl PixelScale {
    /// World units per sprite pixel, horizontally and vertically. The terrain's proportions are
    /// fixed, so a pack whose ratio differs gets slightly non-square pixels, and its sprites
    /// still line up with the levels and hexes they stand on.
    pub fn world_per_pixel(self) -> Vec2 {
        Vec2::new(
            HEX_WIDTH / self.hex_width_px as f32,
            LEVEL_HEIGHT / self.pixels_per_level as f32,
        )
    }

    /// A height in sub-levels as whole pixels, rounded to nearest.
    pub fn sub_levels_to_px(self, sub_levels: u16) -> u32 {
        let (sub, per) = (u64::from(SUB_LEVELS), u64::from(self.pixels_per_level));
        let px = (u64::from(sub_levels) * per + sub / 2) / sub;
        u32::try_from(px).unwrap_or(u32::MAX)
    }

    /// A height in pixels as sub-levels.
    pub fn px_to_sub_levels(self, px: u32) -> f32 {
        (px * SUB_LEVELS) as f32 / self.pixels_per_level as f32
    }
}

/// One animation's length and speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Animation {
    /// Frames, left to right in the sheet.
    pub frames: u32,
    /// Frames per second.
    pub fps: u32,
}

/// A sprite set: frame size, foot anchor and animations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteDef {
    /// The visual key, such as `sprite.test_soldier`.
    pub key: Sym,
    /// Size of one frame in pixels.
    pub frame: UVec2,
    /// The foot point in pixels from the frame's top-left corner, on pixel edges: rows above
    /// `anchor.y` stand above the surface.
    pub anchor: UVec2,
    /// The animations by name; `idle` is always present.
    pub animations: BTreeMap<Sym, Animation>,
}

/// Which frame of which animation to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pose {
    /// The animation actually shown, after falling back to `idle`.
    pub animation: Sym,
    /// The frame within it.
    pub frame: u32,
    /// Whether a one-shot animation has played to its end; `idle` never finishes. A missing
    /// animation plays `idle` once and then finishes.
    pub finished: bool,
}

/// The sheet direction to show for a figure with `facing` (see [`facing`]), seen from camera yaw
/// step `step`.
pub fn direction(facing: Rotation, step: u32) -> u8 {
    ((u32::from(facing.steps()) + step) % 6) as u8
}

impl SpriteDef {
    /// The animations in sheet order.
    pub fn sheet_order(&self) -> Vec<&Sym> {
        let fixed = ANIMATIONS
            .iter()
            .filter_map(|a| self.animations.get_key_value(*a));
        let others = self
            .animations
            .iter()
            .filter(|(k, _)| !ANIMATIONS.contains(&k.as_str()));
        fixed.chain(others).map(|(k, _)| k).collect()
    }

    /// The size in pixels a sheet must have to hold every frame; `None` when it overflows.
    pub fn sheet_size(&self) -> Option<UVec2> {
        let columns = self
            .animations
            .values()
            .map(|a| a.frames)
            .max()
            .unwrap_or(1);
        let rows = u32::try_from(self.animations.len()).ok()?.checked_mul(6)?;
        let width = self.frame.x.checked_mul(columns)?;
        Some(UVec2::new(width, self.frame.y.checked_mul(rows)?))
    }

    /// The animation called `name`, or `idle` when the sprite lacks it. `None` only for a sprite
    /// with no animations at all, which the catalog never builds.
    pub fn animation<'a>(&'a self, name: &str) -> Option<(&'a Sym, &'a Animation)> {
        self.animations
            .get_key_value(name)
            .or_else(|| self.animations.get_key_value(IDLE))
    }

    /// The pose `elapsed` seconds into animation `name`. `idle` loops; any other animation plays
    /// once and holds its last frame. An animation the sprite lacks plays one cycle of `idle`
    /// and then reports finished, so playback waiting on it (a `death`, say) moves on.
    pub fn pose(&self, name: &str, elapsed: f32) -> Pose {
        let looping = name == IDLE;
        let Some((animation, timing)) = self.animation(name) else {
            let animation = Sym::new(IDLE);
            return Pose {
                animation,
                frame: 0,
                finished: !looping,
            };
        };
        let ticks = (elapsed.max(0.0) * timing.fps as f32).floor() as u32;
        let frame = if looping {
            ticks % timing.frames
        } else {
            ticks.min(timing.frames - 1)
        };
        Pose {
            animation: animation.clone(),
            frame,
            finished: !looping && ticks >= timing.frames,
        }
    }

    /// The top-left pixel of a frame in the sheet; `None` for an animation the sprite lacks.
    pub fn cell(&self, animation: &str, direction: u8, frame: u32) -> Option<UVec2> {
        let block = self
            .sheet_order()
            .iter()
            .position(|a| a.as_str() == animation)?;
        let row = 6 * block as u32 + u32::from(direction % 6);
        Some(self.frame * UVec2::new(frame, row))
    }

    /// The texture rectangle `(min, max)` of a frame in a sheet of `sheet` pixels, in `0..1`.
    pub fn uv_rect(&self, sheet: UVec2, pose: &Pose, direction: u8) -> (Vec2, Vec2) {
        let cell = self
            .cell(pose.animation.as_str(), direction, pose.frame)
            .unwrap_or_default();
        let size = sheet.max(UVec2::ONE).as_vec2();
        (cell.as_vec2() / size, (cell + self.frame).as_vec2() / size)
    }
}

#[cfg(test)]
mod tests;
