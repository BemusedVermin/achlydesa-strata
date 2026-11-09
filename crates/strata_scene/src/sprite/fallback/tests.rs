use std::f32::consts::FRAC_PI_3;

use glam::Vec3;
use strata_kernel::{Hex, Rotation, Space, Sym};

use super::*;
use crate::layout::{LEVEL_HEIGHT, surface_point};
use crate::sprite::{SUB_LEVELS, billboard_rotation, direction, foot_point, quad_corners};
use crate::view::IsoView;

/// Three mutually adjacent cells: on flat hexes, lopsided about the middle of their cells when
/// one side is seen end on.
const ELL: [Hex; 3] = [Hex::ORIGIN, Hex::new(1, 0), Hex::new(1, -1)];

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
    let foot = surface_point(Orientation::Pointy, Hex::ORIGIN, 1);
    let corners = quad_corners(&fallback.def, scale);
    let frame_h = fallback.def.frame.y as f32;
    // A yaw at every camera step, and some between steps while the camera eases.
    let yaws = (0..12)
        .map(|k| k as f32 * FRAC_PI_3 / 2.0)
        .chain([0.3, -2.0]);
    for yaw in yaws {
        let view = IsoView {
            yaw,
            ..IsoView::default()
        };
        let rotation = billboard_rotation(yaw);
        // A point on the drawn quad above the foot anchor, `row` pixel edges down from the
        // frame's top, in the world.
        let across = fallback.def.anchor.x as f32 / fallback.def.frame.x as f32;
        let on_quad = |row: u32| {
            let up = (frame_h - row as f32) / frame_h;
            let bottom = corners[0].lerp(corners[1], across);
            foot + rotation * (bottom + (corners[3] - corners[0]) * up)
        };
        // A column two levels tall, as the terrain draws it.
        let column = screen_height(&view, foot, foot + Vec3::Y * 2.0 * LEVEL_HEIGHT);
        for d in 0..6 {
            let (top, bottom) = opaque_rows(&fallback, d);
            assert_eq!(bottom, fallback.def.anchor.y, "it stands on its foot point");
            assert!((on_quad(bottom) - foot).length() < 1e-5, "yaw {yaw}");
            let drawn = screen_height(&view, on_quad(bottom), on_quad(top));
            assert!(
                (drawn - column).abs() < 1e-3,
                "yaw {yaw}, direction {d}: {drawn} vs {column}"
            );
        }
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

/// The opaque columns of direction `d`'s frame as world offsets `(left, right)` from the foot.
fn drawn_extent(fallback: &FallbackSprite, scale: PixelScale, d: u32) -> (f32, f32) {
    let (first, last) = opaque_columns(fallback, d);
    let anchor = fallback.def.anchor.x as f32;
    let px = scale.world_per_pixel().x;
    ((first as f32 - anchor) * px, (last as f32 - anchor) * px)
}

#[test]
fn a_fallback_never_spills_wider_than_its_footprint() {
    let scale = PixelScale::default();
    let line = [Hex::ORIGIN, Hex::new(1, 0)];
    for footprint in [&[Hex::ORIGIN][..], &line[..]] {
        let fallback = sprite((0, 12), footprint, scale);
        for d in 0..6u8 {
            let (lo, hi) = extent(Orientation::Pointy, footprint, d);
            let (left, right) = drawn_extent(&fallback, scale, u32::from(d));
            assert!(lo - 1e-4 <= left && right <= hi + 1e-4, "direction {d}");
            let (first, last) = opaque_columns(&fallback, u32::from(d));
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
fn a_lopsided_footprint_is_covered_from_where_its_billboard_stands() {
    let scale = PixelScale::default();
    let px = scale.world_per_pixel().x;
    let mut off_centre = false;
    for orientation in [Orientation::Pointy, Orientation::Flat] {
        let key = Sym::new("sprite.test_soldier");
        let fallback = fallback_sprite(key, (0, 12), &ELL, orientation, RED, scale);
        for r in 0..6 {
            let rotation = Rotation::from_steps(r);
            let cells: Vec<Hex> = ELL.iter().map(|c| c.rotate(rotation)).collect();
            // Where the billboard stands, and the cells' extent across the screen around it.
            let foot = foot_point(orientation, Space::at(Hex::ORIGIN, 0), &cells).x;
            let xs = cells.iter().map(|c| orientation.center(*c).0 - foot);
            let (min, max) = xs.fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
            let half = INSET * orientation.hex_span() / 2.0;
            let (lo, hi) = (min - half, max + half);
            let d = direction(facing(&ELL, rotation), 0);
            let (left, right) = drawn_extent(&fallback, scale, u32::from(d));
            assert!(
                lo - 1e-4 <= left && right <= hi + 1e-4,
                "{orientation:?} rotation {r}: drawn {left}..{right} outside {lo}..{hi}"
            );
            assert!(
                left - lo <= px + 1e-4 && hi - right <= px + 1e-4,
                "fills the extent"
            );
            off_centre |= (lo + hi).abs() > 0.1;
        }
    }
    assert!(
        off_centre,
        "the three cells are lopsided seen from some side"
    );
}

#[test]
fn each_frame_is_the_footprint_as_the_camera_sees_it() {
    for footprint in [&ELL[..], &[Hex::ORIGIN, Hex::new(1, 0)][..]] {
        for orientation in [Orientation::Pointy, Orientation::Flat] {
            for f in 0..6 {
                for s in 0..6u32 {
                    let view = IsoView {
                        yaw: s as f32 * FRAC_PI_3,
                        ..IsoView::default()
                    };
                    let turn = Rotation::from_steps(f);
                    let cells: Vec<Hex> = footprint.iter().map(|c| c.rotate(turn)).collect();
                    let foot = foot_point(orientation, Space::at(Hex::ORIGIN, 0), &cells);
                    let xs = cells.iter().map(|c| {
                        let (x, z) = orientation.center(*c);
                        (Vec3::new(x, 0.0, z) - foot).dot(view.right())
                    });
                    let (lo, hi) = xs.fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
                    let half = INSET * orientation.hex_span() / 2.0;
                    let d = direction(facing(footprint, turn), s);
                    let (a, b) = extent(orientation, footprint, d);
                    assert!((lo - half - a).abs() < 1e-4 && (hi + half - b).abs() < 1e-4);
                }
            }
        }
    }
}

#[test]
fn directions_differ_and_the_sheet_is_alpha_cutout() {
    let fallback = sprite((0, 36), &[Hex::ORIGIN], PixelScale::default());
    assert_eq!(Some(fallback.size), fallback.def.sheet_size());
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
