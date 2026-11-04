use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_3;

use glam::Vec3;
use strata_kernel::{DefId, Hex, Rotation, Shape, Space, Sym};
use strata_model::board::{BoardDefs, compile};
use strata_model::{MapConstraints, MapDef, PlacedPiece, SourceLoc, Text, TileDef};

use super::layout::{Orientation, surface_point};
use super::*;

const GRASS: DefId = DefId(1);
const ROCK: DefId = DefId(2);

struct Defs(BTreeMap<Sym, TileDef>);

impl BoardDefs for Defs {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        self.0.get(name)
    }
    fn feature(&self, _: &Sym) -> Option<&strata_model::FeatureDef> {
        None
    }
}

fn tile(terrain: DefId, thickness: u8) -> TileDef {
    TileDef {
        id: DefId(0),
        name: Text::default(),
        terrain,
        thickness,
        shape: Shape::new([Hex::ORIGIN]),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    }
}

fn piece(name: &str, at: (i32, i32), level: u8) -> PlacedPiece {
    PlacedPiece {
        piece: name.into(),
        at: Hex::new(at.0, at.1),
        rot: Rotation::from_steps(0),
        level,
    }
}

/// A one-level slab at `(0, 0)`, a three-level block at `(6, 0)` and a bridge deck over
/// `(-2..=2, 3)`.
fn board(orientation: Orientation) -> SceneBoard {
    let mut tiles = BTreeMap::new();
    tiles.insert("slab".into(), tile(GRASS, 1));
    tiles.insert("block".into(), tile(ROCK, 3));
    tiles.insert("deck".into(), tile(ROCK, 1));
    let mut pieces = vec![piece("slab", (0, 0), 0), piece("block", (6, 0), 0)];
    pieces.push(piece("slab", (6, 0), 3));
    for q in -2..=2 {
        pieces.push(piece("slab", (q, 3), 0));
        pieces.push(piece("deck", (q, 3), 4));
    }
    let map = MapDef {
        id: DefId(0),
        name: Text::default(),
        authors: Vec::new(),
        orientation: "pointy".into(),
        pieces,
        features: Vec::new(),
        zones: Default::default(),
        relic_spots: Default::default(),
        constraints: MapConstraints {
            inventory: None,
            require_support: false,
        },
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    SceneBoard {
        board: compile(&map, &Defs(tiles)).unwrap(),
        orientation,
    }
}

const ORIENTATIONS: [Orientation; 2] = [Orientation::Pointy, Orientation::Flat];

fn camera_at(step: u32) -> IsoView {
    IsoView {
        step,
        yaw: step as f32 * FRAC_PI_3,
        ..IsoView::default()
    }
}

/// The view ray through a world point, as the camera at `step` would cast it.
fn view_ray(step: u32, target: Vec3) -> (Vec3, Vec3) {
    let view = camera_at(step);
    let direction = view.forward();
    (target - direction * 50.0, direction)
}

#[test]
fn hex_round_trips_through_the_world() {
    for orientation in ORIENTATIONS {
        for q in -20..=20 {
            for r in -20..=20 {
                let hex = Hex::new(q, r);
                let (x, z) = orientation.center(hex);
                assert_eq!(orientation.hex_at(x, z), hex, "{orientation:?} {hex:?}");
            }
        }
    }
}

#[test]
fn views_down_at_the_surface_from_every_yaw() {
    for orientation in ORIENTATIONS {
        let board = board(orientation);
        let cases = [
            (Hex::new(0, 0), 1),
            (Hex::new(6, 0), 4),
            (Hex::new(0, 3), 5),
        ];
        for step in 0..6 {
            for (hex, z) in cases {
                let (origin, dir) = view_ray(step, surface_point(orientation, hex, z));
                let picked = pick(&board, origin, dir);
                assert_eq!(
                    picked,
                    Some(Space::at(hex, z)),
                    "{orientation:?} yaw {step}"
                );
            }
        }
    }
}

#[test]
fn a_ray_through_the_gap_picks_the_ground_under_a_bridge() {
    for orientation in ORIENTATIONS {
        let board = board(orientation);
        let hex = Hex::new(0, 3);
        let target = surface_point(orientation, hex, 1);
        for step in 0..6 {
            let angle = step as f32 * FRAC_PI_3;
            let dir = Vec3::new(angle.cos(), -0.1, angle.sin()).normalize();
            let origin = target - dir * 5.2;
            let picked = pick(&board, origin, dir);
            assert_eq!(
                picked,
                Some(Space::at(hex, 1)),
                "{orientation:?} yaw {step}"
            );
        }
    }
}

#[test]
fn a_side_wall_picks_the_top_of_its_column() {
    for orientation in ORIENTATIONS {
        let board = board(orientation);
        let hex = Hex::new(6, 0);
        let wall = surface_point(orientation, hex, 1);
        for step in 0..6 {
            let angle = step as f32 * FRAC_PI_3;
            let dir = Vec3::new(angle.cos(), -0.1, angle.sin()).normalize();
            let picked = pick(&board, wall - dir * 5.0, dir);
            assert_eq!(
                picked,
                Some(Space::at(hex, 4)),
                "{orientation:?} yaw {step}"
            );
        }
    }
}

#[test]
fn rays_that_miss_pick_nothing() {
    let board = board(Orientation::Pointy);
    let up = pick(&board, Vec3::ZERO, Vec3::Y);
    let away = pick(
        &board,
        Vec3::new(500.0, 20.0, 500.0),
        Vec3::new(1.0, -1.0, 1.0),
    );
    assert_eq!((up, away), (None, None));
}

#[test]
fn yaw_eases_the_short_way_round() {
    let mut camera = camera_at(0);
    camera.turn(-1);
    assert_eq!(camera.step, 5);
    for _ in 0..200 {
        camera.ease(0.016);
    }
    let target = -FRAC_PI_3;
    assert!((camera.yaw - target).abs() < 1e-2 || (camera.yaw - 5.0 * FRAC_PI_3).abs() < 1e-2);
}

#[test]
fn zoom_stays_on_whole_steps_in_range() {
    let mut camera = IsoView::default();
    camera.zoom_by(-10);
    assert_eq!(camera.zoom, 1);
    camera.zoom_by(100);
    assert_eq!(camera.zoom, 6);
}
