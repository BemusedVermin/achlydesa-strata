//! Property and unit tests for hex coordinates (T-002).

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use strata_kernel::{Axis, DIRECTIONS, Hex, Rotation, Space};

fn hex() -> impl Strategy<Value = Hex> {
    (-1000..1000, -1000..1000).prop_map(|(q, r)| Hex::new(q, r))
}

fn rotation() -> impl Strategy<Value = Rotation> {
    (0u8..6).prop_map(|k| Rotation::new(k).unwrap())
}

fn axis() -> impl Strategy<Value = Axis> {
    prop_oneof![Just(Axis::Q), Just(Axis::R), Just(Axis::S)]
}

proptest! {
    #[test]
    fn distance_is_symmetric(a in hex(), b in hex()) {
        prop_assert_eq!(a.distance(b), b.distance(a));
    }

    #[test]
    fn distance_satisfies_triangle_inequality(a in hex(), b in hex(), c in hex()) {
        prop_assert!(a.distance(c) <= a.distance(b) + b.distance(c));
    }

    #[test]
    fn distance_is_zero_only_for_equal_hexes(a in hex(), b in hex()) {
        prop_assert_eq!(a.distance(b) == 0, a == b);
    }

    #[test]
    fn neighbors_are_at_distance_one_and_distinct(a in hex()) {
        let neighbors = a.neighbors();
        for n in neighbors {
            prop_assert_eq!(a.distance(n), 1);
        }
        prop_assert_eq!(neighbors.iter().collect::<BTreeSet<_>>().len(), 6);
    }

    #[test]
    fn rotation_preserves_distance(a in hex(), b in hex(), k in rotation()) {
        prop_assert_eq!(a.rotate(k).distance(b.rotate(k)), a.distance(b));
    }

    #[test]
    fn rotation_about_center_preserves_distance(a in hex(), b in hex(), c in hex(), k in rotation()) {
        prop_assert_eq!(
            a.rotate_about(c, k).distance(b.rotate_about(c, k)),
            a.distance(b)
        );
        prop_assert_eq!(a.rotate_about(c, k).distance(c), a.distance(c));
    }

    #[test]
    fn six_sixty_degree_rotations_are_identity(a in hex(), c in hex()) {
        let one = Rotation::from_steps(1);
        let mut turned = a;
        let mut about = a;
        for _ in 0..6 {
            turned = turned.rotate(one);
            about = about.rotate_about(c, one);
        }
        prop_assert_eq!(turned, a);
        prop_assert_eq!(about, a);
    }

    #[test]
    fn rotations_compose_and_invert(a in hex(), j in rotation(), k in rotation()) {
        prop_assert_eq!(a.rotate(j).rotate(k), a.rotate(j.then(k)));
        prop_assert_eq!(a.rotate(k).rotate(k.inverse()), a);
    }

    #[test]
    fn reflection_twice_is_identity(a in hex(), axis in axis()) {
        prop_assert_eq!(a.reflect(axis).reflect(axis), a);
    }

    #[test]
    fn reflection_preserves_distance(a in hex(), b in hex(), axis in axis()) {
        prop_assert_eq!(a.reflect(axis).distance(b.reflect(axis)), a.distance(b));
    }

    #[test]
    fn ring_has_six_k_distinct_cells_at_distance_k(c in hex(), k in 1u32..40) {
        let ring = c.ring(k);
        prop_assert_eq!(ring.len(), 6 * k as usize);
        prop_assert_eq!(ring.iter().collect::<BTreeSet<_>>().len(), ring.len());
        for cell in ring {
            prop_assert_eq!(c.distance(cell), k);
        }
    }

    #[test]
    fn spiral_covers_the_disc_once(c in hex(), k in 0u32..20) {
        let spiral = c.spiral(k);
        let k = k as usize;
        prop_assert_eq!(spiral.len(), 3 * k * (k + 1) + 1);
        prop_assert_eq!(spiral.iter().collect::<BTreeSet<_>>().len(), spiral.len());
    }

    #[test]
    fn spiral_and_ring_are_deterministic(c in hex(), k in 0u32..20) {
        prop_assert_eq!(c.ring(k), c.ring(k));
        prop_assert_eq!(c.spiral(k), c.spiral(k));
    }

    #[test]
    fn serde_round_trips(a in hex(), z in 0u32..100) {
        let json = serde_json::to_string(&a).unwrap();
        prop_assert_eq!(serde_json::from_str::<Hex>(&json).unwrap(), a);
        let space = Space::at(a, z);
        let json = serde_json::to_string(&space).unwrap();
        prop_assert_eq!(serde_json::from_str::<Space>(&json).unwrap(), space);
    }
}

#[test]
fn cube_coordinates_sum_to_zero() {
    let h = Hex::new(3, -5);
    assert_eq!(h.q + h.r + h.s(), 0);
}

#[test]
fn rotating_a_direction_one_step_gives_the_next_direction() {
    let one = Rotation::from_steps(1);
    for i in 0..6 {
        assert_eq!(DIRECTIONS[i].rotate(one), DIRECTIONS[(i + 1) % 6]);
    }
}

#[test]
fn ring_order_is_fixed() {
    assert_eq!(Hex::ORIGIN.ring(0), vec![Hex::ORIGIN]);
    assert_eq!(
        Hex::ORIGIN.ring(1),
        vec![
            Hex::new(0, -1),
            Hex::new(1, -1),
            Hex::new(1, 0),
            Hex::new(0, 1),
            Hex::new(-1, 1),
            Hex::new(-1, 0),
        ]
    );
}

#[test]
fn rotation_is_validated() {
    assert!(Rotation::new(6).is_none());
    assert_eq!(Rotation::from_steps(-1), Rotation::new(5).unwrap());
    assert!(serde_json::from_str::<Rotation>("6").is_err());
    assert_eq!(
        serde_json::from_str::<Rotation>("4").unwrap(),
        Rotation::new(4).unwrap()
    );
}

#[test]
fn hex_and_space_work_as_ordered_map_keys() {
    let mut hexes = BTreeMap::new();
    hexes.insert(Hex::new(1, 0), "a");
    hexes.insert(Hex::new(0, 5), "b");
    assert_eq!(hexes.keys().next(), Some(&Hex::new(0, 5)));

    let mut spaces = BTreeMap::new();
    spaces.insert(Space::new(0, 0, 4), 1);
    spaces.insert(Space::new(0, 0, 1), 2);
    assert_eq!(spaces.keys().next(), Some(&Space::new(0, 0, 1)));
    assert_eq!(Space::new(2, -3, 1).hex(), Hex::new(2, -3));
}
