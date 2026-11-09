//! Generated placeholder sprites for figures without art (design §13.4).
//!
//! The placeholder is a capsule in the figure's color, as tall as its hit zone and as wide as its
//! footprint (inset to 80%) seen from each of the six directions, with a light visor on the side
//! it faces. It is drawn at the pack's pixel scale, so the sprite lint measures it like any other
//! sprite and it passes by construction.

use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_3;

use glam::UVec2;
use strata_kernel::{Hex, Rotation, Sym};

use super::{Animation, IDLE, PixelScale, SpriteDef};
use crate::layout::Orientation;

/// An sRGB color.
pub type Rgb = [u8; 3];

/// How much of each hex a figure's volume covers across (design §7.6).
const INSET: f32 = 0.8;

/// A generated sprite: its layout and its sheet as RGBA pixels, row by row from the top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FallbackSprite {
    /// The frame layout: one `idle` frame in each of the six directions.
    pub def: SpriteDef,
    /// The sheet's size in pixels.
    pub size: UVec2,
    /// Four bytes per pixel; alpha is 0 or 255, for alpha-cutout drawing.
    pub rgba: Vec<u8>,
}

impl FallbackSprite {
    /// Whether the pixel at `(x, y)` of the sheet is opaque.
    pub fn opaque(&self, x: u32, y: u32) -> bool {
        let at = ((y * self.size.x + x) * 4 + 3) as usize;
        self.rgba.get(at).is_some_and(|a| *a > 0)
    }
}

/// Parses a `#RRGGBB` color, as factions declare them.
pub fn parse_color(text: &str) -> Option<Rgb> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// A distinct color for a seat, for units without a faction.
pub fn seat_color(seat: u8) -> Rgb {
    const SEATS: [Rgb; 6] = [
        [0x3a, 0x6e, 0xc9],
        [0xc9, 0x45, 0x3a],
        [0x3a, 0xa3, 0x5c],
        [0xd1, 0xa3, 0x2e],
        [0x8e, 0x4a, 0xb8],
        [0x2e, 0xa8, 0xb0],
    ];
    SEATS[usize::from(seat) % SEATS.len()]
}

/// The capsule's width in world units seen from direction `d`: the footprint turned to facing
/// `d`, viewed from camera step 0, whose screen right is `+x`.
fn span(orientation: Orientation, footprint: &[Hex], d: u8) -> f32 {
    let turn = Rotation::from_steps(i32::from(d));
    let xs = footprint
        .iter()
        .map(|c| orientation.center(c.rotate(turn)).0);
    let (min, max) = xs.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
    let centers = if footprint.is_empty() { 0.0 } else { max - min };
    centers + INSET * orientation.hex_span()
}

/// Generates the placeholder for a figure whose hit zone is `hit_zone` (sub-levels) and whose
/// footprint cells around its anchor are `footprint`, in `color`, at the pixel `scale`.
pub fn fallback_sprite(
    key: Sym,
    hit_zone: (u16, u16),
    footprint: &[Hex],
    orientation: Orientation,
    color: Rgb,
    scale: PixelScale,
) -> FallbackSprite {
    let top = scale.sub_levels_to_px(hit_zone.1).max(1);
    let bottom = scale.sub_levels_to_px(hit_zone.0).min(top - 1);
    let px = scale.world_per_pixel().x;
    let widths: Vec<u32> = (0..6)
        .map(|d| ((span(orientation, footprint, d) / px).floor() as u32).max(1))
        .collect();
    let max = widths.iter().copied().max().unwrap_or(1);
    let w = max + max % 2;
    let size = UVec2::new(w, 6 * top);
    let mut rgba = vec![0; (size.x * size.y * 4) as usize];
    for (d, width) in widths.into_iter().enumerate() {
        // Same parity as the frame, so the capsule is centered on the foot point.
        let width = width - width % 2;
        let width = if width == 0 { 2 } else { width };
        let capsule = Capsule {
            center: w as f32 / 2.0,
            half_width: width as f32 / 2.0,
            rows: (0, top - bottom),
            facing: orientation.first_direction_angle() + d as f32 * FRAC_PI_3,
        };
        for y in 0..top {
            for x in 0..w {
                if let Some(rgb) = capsule.shade(x, y, color) {
                    let at = (((d as u32 * top + y) * w + x) * 4) as usize;
                    rgba[at..at + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
            }
        }
    }
    let animations = BTreeMap::from([(Sym::new(IDLE), Animation { frames: 1, fps: 1 })]);
    let def = SpriteDef {
        key,
        frame: UVec2::new(w, top),
        anchor: UVec2::new(w / 2, top),
        animations,
    };
    FallbackSprite { def, size, rgba }
}

/// One direction's capsule within its frame.
struct Capsule {
    center: f32,
    half_width: f32,
    /// The opaque rows, `[from, to)`.
    rows: (u32, u32),
    /// The facing's angle from `+x` toward `+z`, the viewer's side at camera step 0.
    facing: f32,
}

impl Capsule {
    /// Whether the pixel center `(x, y)` lies inside the rounded outline.
    fn inside(&self, x: f32, y: f32) -> bool {
        let half_height = (self.rows.1 - self.rows.0) as f32 / 2.0;
        let middle = self.rows.0 as f32 + half_height;
        let radius = self.half_width.min(half_height);
        let dx = ((x - self.center).abs() - (self.half_width - radius)).max(0.0);
        let dy = ((y - middle).abs() - (half_height - radius)).max(0.0);
        dx * dx + dy * dy <= radius * radius + 1e-3
    }

    /// The color of pixel `(x, y)`, or `None` where it is transparent: a darker outline, a
    /// lighter visor toward the facing when it faces the viewer or sideways, and `color` inside.
    fn shade(&self, x: u32, y: u32, color: Rgb) -> Option<Rgb> {
        let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
        if !self.inside(cx, cy) {
            return None;
        }
        let edge = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)]
            .iter()
            .any(|(dx, dy)| !self.inside(cx + dx, cy + dy));
        let mix = |toward: u8, amount: u16| {
            color.map(|c| ((u16::from(c) * (8 - amount) + u16::from(toward) * amount) / 8) as u8)
        };
        if edge {
            return Some(mix(0, 4));
        }
        let (sin, cos) = self.facing.sin_cos();
        let height = (self.rows.1 - self.rows.0) as f32;
        let visor_x = self.center + cos * self.half_width * 0.5;
        let visor_y = self.rows.0 as f32 + height / 6.0;
        let visor = sin > -0.01
            && (cx - visor_x).abs() <= (self.half_width / 4.0).max(1.0)
            && (cy - visor_y).abs() <= (height / 16.0).max(0.5);
        Some(if visor { mix(255, 5) } else { color })
    }
}

#[cfg(test)]
mod tests;
