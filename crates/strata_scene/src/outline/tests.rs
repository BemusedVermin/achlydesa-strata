use glam::Vec3;
use proptest::prelude::*;
use strata_kernel::{Hex, Point, Point3, SUB_LEVELS, Seg3, Space};

use super::*;
use crate::layout::{LEVEL_HEIGHT, Orientation, surface_point};

const BOTH: [Orientation; 2] = [Orientation::Pointy, Orientation::Flat];

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn a_hex_center_at_whole_levels_is_its_surface_point() {
    for orientation in BOTH {
        let hex = Hex::new(2, -3);
        let at = lattice_point(orientation, Point3::new(Point::center(hex), 3 * SUB_LEVELS));
        assert!(at.distance(surface_point(orientation, hex, 3)) < 1e-4);
    }
}

#[test]
fn a_sub_level_is_a_twelfth_of_a_level() {
    let at = lattice_point(Orientation::Pointy, Point3::new(Point::new(0, 0), 6));
    assert!(close(at.y, LEVEL_HEIGHT / 2.0));
}

#[test]
fn a_witness_ray_runs_between_its_lattice_points() {
    let seg = Seg3::new(
        Point3::new(Point::new(0, 0), 10),
        Point3::new(Point::new(30, 0), 22),
    );
    let [a, b] = segment(Orientation::Pointy, seg);
    let far = surface_point(Orientation::Pointy, Hex::new(2, 0), 0);
    assert!(close(a.x, 0.0) && close(a.y, 10.0 / 12.0 * LEVEL_HEIGHT));
    assert!(close(b.x, far.x) && close(b.z, far.z) && close(b.y, 22.0 / 12.0 * LEVEL_HEIGHT));
}

#[test]
fn a_volume_has_two_rings_and_six_uprights_per_cell() {
    let cells = [Hex::new(0, 0), Hex::new(1, 0)];
    let lines = volume_outline(Orientation::Pointy, &cells, 12, 30);
    assert_eq!(lines.len(), 36);
    let (low, high) = (LEVEL_HEIGHT, 30.0 / 12.0 * LEVEL_HEIGHT);
    let flat = |l: &Line| close(l[0].y, l[1].y);
    let rings = lines.iter().filter(|l| flat(l));
    assert_eq!(rings.clone().filter(|l| close(l[0].y, low)).count(), 12);
    assert_eq!(rings.filter(|l| close(l[0].y, high)).count(), 12);
    let uprights = lines.iter().filter(|l| !flat(l));
    assert!(
        uprights
            .clone()
            .all(|l| close(l[0].y, low) && close(l[1].y, high))
    );
    assert_eq!(uprights.count(), 12);
}

#[test]
fn a_ring_and_a_route_lie_just_above_the_surface() {
    let space = Space::new(1, 1, 2);
    let lift = 2.0 * LEVEL_HEIGHT + LIFT;
    let lines = ring(Orientation::Flat, space, 0.5);
    assert_eq!(lines.len(), 6);
    let center = surface_point(Orientation::Flat, space.hex(), 2);
    for [a, b] in &lines {
        assert!(close(a.y, lift) && close(b.y, lift));
        assert!(close(
            Vec3::new(a.x - center.x, 0.0, a.z - center.z).length(),
            0.5
        ));
    }
    let path = [Space::new(1, 0, 1), Space::new(2, 0, 1)];
    let steps = route(Orientation::Pointy, Space::new(0, 0, 1), &path);
    assert_eq!(steps.len(), 2);
    assert!(steps[0][1].distance(steps[1][0]) < 1e-6);
    assert!(close(steps[1][1].y, LEVEL_HEIGHT + LIFT));
    assert!(route(Orientation::Pointy, space, &[]).is_empty());
}

proptest! {
    #[test]
    fn volume_corners_sit_at_four_fifths_of_a_corner(q in -20i32..20, r in -20i32..20, o in 0usize..2) {
        let orientation = BOTH[o];
        let hex = Hex::new(q, r);
        let (cx, cz) = orientation.center(hex);
        for [a, b] in volume_outline(orientation, &[hex], 0, 12) {
            for p in [a, b] {
                let d = Vec3::new(p.x - cx, 0.0, p.z - cz).length();
                prop_assert!(close(d, 0.8), "{d}");
            }
        }
    }

    #[test]
    fn a_ring_is_a_regular_hexagon(q in -20i32..20, r in -20i32..20, scale in 0.1f32..1.0) {
        let lines = ring(Orientation::Pointy, Space::new(q, r, 0), scale);
        for [a, b] in &lines {
            prop_assert!(close(a.distance(*b), scale), "side {} for scale {scale}", a.distance(*b));
        }
    }
}
