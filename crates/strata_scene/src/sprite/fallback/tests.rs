use std::f32::consts::FRAC_PI_3;

use glam::Vec3;
use strata_kernel::{Hex, Rotation, Sym};

use super::*;
use crate::layout::{LEVEL_HEIGHT, surface_point};
use crate::sprite::{SUB_LEVELS, direction, quad_corners};
use crate::view::IsoView;

const RED: Rgb = [200, 40, 40];

fn sprite(hit_zone: (u16, u16), footprint: &[Hex], scale: PixelScale) -> FallbackSprite {
    let key = Sym::new("sprite.test_soldier");
    fallback_sprite(key, hit_zone, footprint, Orientation::Pointy, RED, scale)
}

/// The opaque rows `(first, last + 1)` of direction `d`'s frame, counted from the frame's top.
fn opaque_rows(sprite: &FallbackSprite, d: u32) -> (u32, u32) {
    let h = sprite.def.frame.y;
    let rows: Vec<u32> = (0..h)
        .filter(|y| (0..sprite.size.x).any(|x| sprite.opaque(x, d * h + y)))
        .collect();
    (rows[0], rows[rows.len() - 1] + 1)
}

/// The opaque columns `(first, last + 1)` of direction `d`'s frame.
fn opaque_columns(sprite: &FallbackSprite, d: u32) -> (u32, u32) {
    let h = sprite.def.frame.y;
    let cols: Vec<u32> = (0..sprite.size.x)
        .filter(|x| (0..h).any(|y| sprite.opaque(*x, d * h + y)))
        .collect();
    (cols[0], cols[cols.len() - 1] + 1)
}

/// Screen pixels between two world points along the screen's vertical.
fn screen_height(view: &IsoView, low: Vec3, high: Vec3) -> f32 {
    let up = view.right().cross(view.forward());
    (high - low).dot(up) / view.scale()
}

#[test]
fn a_fallback_is_as_tall_on_screen_as_its_hit_zone() {
    let scale = PixelScale::default();
    let fallback = sprite((0, 24), &[Hex::ORIGIN], scale);
    let view = IsoView::default();
    let foot = surface_point(Orientation::Pointy, Hex::ORIGIN, 1);
    // A column two levels tall, as the terrain draws it.
    let column = screen_height(&view, foot, foot + Vec3::Y * 2.0 * LEVEL_HEIGHT);
    let px = scale.world_per_pixel();
    for d in 0..6 {
        let (top, bottom) = opaque_rows(&fallback, d);
        assert_eq!(bottom, fallback.def.anchor.y, "it stands on its foot point");
        let corners = quad_corners(&fallback.def, scale);
        let opaque_top = corners[0].y + (fallback.def.frame.y - top) as f32 * px.y;
        let drawn = screen_height(&view, foot, foot + Vec3::Y * opaque_top);
        assert!(
            (drawn - column).abs() < 1e-3,
            "direction {d}: {drawn} vs {column}"
        );
    }
}

#[test]
fn a_fallback_height_matches_the_hit_zone_at_every_pack_scale() {
    let scales = [(12, 64), (16, 64), (8, 48), (24, 96), (10, 64)];
    for (pixels_per_level, hex_width_px) in scales {
        let scale = PixelScale {
            pixels_per_level,
            hex_width_px,
        };
        for top in 1..=60u16 {
            for bottom in [0, top / 3] {
                let fallback = sprite((bottom, top), &[Hex::ORIGIN], scale);
                let (first, last) = opaque_rows(&fallback, 0);
                let anchor = fallback.def.anchor.y;
                let height = scale.px_to_sub_levels(anchor - first);
                let lift = scale.px_to_sub_levels(anchor - last);
                // The lint allows one sub-level; rounding to whole pixels stays within half a pixel.
                let half_pixel = SUB_LEVELS as f32 / pixels_per_level as f32 / 2.0;
                assert!(
                    (height - f32::from(top)).abs() <= half_pixel + 1e-4,
                    "{scale:?} top {top}"
                );
                assert!(
                    height - f32::from(top) <= 1.0,
                    "within the lint's tolerance"
                );
                if bottom > 0 && scale.sub_levels_to_px(bottom) < scale.sub_levels_to_px(top) {
                    assert!((lift - f32::from(bottom)).abs() <= half_pixel + 1e-4);
                }
                if pixels_per_level % 12 == 0 {
                    assert_eq!(
                        height,
                        f32::from(top),
                        "exact when sub-levels are whole pixels"
                    );
                }
            }
        }
    }
}

#[test]
fn a_fallback_never_spills_wider_than_its_footprint() {
    let scale = PixelScale::default();
    let line = [Hex::ORIGIN, Hex::new(1, 0)];
    for footprint in [&[Hex::ORIGIN][..], &line[..]] {
        let fallback = sprite((0, 12), footprint, scale);
        for d in 0..6u8 {
            let (first, last) = opaque_columns(&fallback, u32::from(d));
            let width = (last - first) as f32 * scale.world_per_pixel().x;
            assert!(width <= span(Orientation::Pointy, footprint, d) + 1e-4);
            let anchor = fallback.def.anchor.x;
            assert_eq!(anchor - first, last - anchor, "centered on the foot point");
        }
    }
    let wide = sprite((0, 12), &line, scale);
    let width = |d| opaque_columns(&wide, d).1 - opaque_columns(&wide, d).0;
    assert!(
        width(0) > width(1) + 10,
        "a line seen across is wider than seen at an angle"
    );
    assert_eq!(width(0), width(3));
}

#[test]
fn each_frame_is_the_footprint_as_the_camera_sees_it() {
    let footprint = [Hex::ORIGIN, Hex::new(1, 0), Hex::new(1, -1)];
    for orientation in [Orientation::Pointy, Orientation::Flat] {
        for f in 0..6 {
            for s in 0..6u32 {
                let view = IsoView {
                    yaw: s as f32 * FRAC_PI_3,
                    ..IsoView::default()
                };
                let turn = Rotation::from_steps(f);
                let xs = footprint.iter().map(|c| {
                    let (x, z) = orientation.center(c.rotate(turn));
                    Vec3::new(x, 0.0, z).dot(view.right())
                });
                let (lo, hi) = xs.fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
                let seen = hi - lo + INSET * orientation.hex_span();
                let d = direction(turn, s);
                assert!((seen - span(orientation, &footprint, d)).abs() < 1e-4);
            }
        }
    }
}

#[test]
fn directions_differ_and_the_sheet_is_alpha_cutout() {
    let fallback = sprite((0, 36), &[Hex::ORIGIN], PixelScale::default());
    assert_eq!(fallback.size, fallback.def.sheet_size());
    assert_eq!(fallback.def.animations.len(), 1);
    let frame = |d: u32| {
        let bytes = (fallback.size.x * fallback.def.frame.y * 4) as usize;
        fallback.rgba[d as usize * bytes..(d as usize + 1) * bytes].to_vec()
    };
    assert_ne!(frame(0), frame(3), "facing right and facing left differ");
    assert_ne!(frame(1), frame(4), "front and back differ");
    assert!(fallback.rgba.chunks(4).all(|p| p[3] == 0 || p[3] == 255));
    let inner = fallback
        .rgba
        .chunks(4)
        .filter(|p| p[3] == 255 && p[..3] == RED)
        .count();
    assert!(inner > 0, "the body is the figure's color");
}

#[test]
fn colors_come_from_the_faction_or_the_seat() {
    assert_eq!(parse_color("#B5893a"), Some([0xb5, 0x89, 0x3a]));
    assert_eq!(parse_color("B5893A"), None);
    assert_eq!(parse_color("#B589"), None);
    assert_eq!(parse_color("#GG893A"), None);
    assert_ne!(seat_color(0), seat_color(1));
    assert_eq!(seat_color(0), seat_color(6));
}
