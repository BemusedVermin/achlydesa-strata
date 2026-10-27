//! Tests for exact segment traversal and inset clipping (T-006).

use proptest::prelude::*;
use strata_kernel::{
    Crossing, DIRECTIONS, Frac, Hex, LATTICE, Point, Rotation, clip_inset, traverse,
};

fn point() -> impl Strategy<Value = Point> {
    (-150i64..150, -150i64..150).prop_map(|(q, r)| Point::new(q, r))
}

fn rotation() -> impl Strategy<Value = Rotation> {
    (0u8..6).prop_map(|k| Rotation::new(k).unwrap())
}

fn rotate_point(p: Point, rotation: Rotation) -> Point {
    let h = Hex::new(p.q as i32, p.r as i32).rotate(rotation);
    Point::new(h.q.into(), h.r.into())
}

fn hexes(crossings: &[Crossing]) -> Vec<Hex> {
    crossings.iter().map(|c| c.hex).collect()
}

fn frac(n: i128, d: i128) -> Frac {
    Frac::new(n, d)
}

fn crossing(q: i32, r: i32, t_in: Frac, t_out: Frac) -> Crossing {
    Crossing {
        hex: Hex::new(q, r),
        t_in,
        t_out,
    }
}

proptest! {
    #[test]
    fn crossings_are_ordered_and_contiguous(a in point(), b in point()) {
        let crossings = traverse(a, b);
        for c in &crossings {
            prop_assert!(Frac::ZERO <= c.t_in && c.t_in < c.t_out && c.t_out <= Frac::ONE);
        }
        for pair in crossings.windows(2) {
            prop_assert!(pair[0].t_out <= pair[1].t_in);
            if pair[0].t_out == pair[1].t_in {
                prop_assert_eq!(pair[0].hex.distance(pair[1].hex), 1);
            }
        }
    }

    #[test]
    fn reversing_the_segment_reverses_the_crossings(a in point(), b in point()) {
        let forward: Vec<Crossing> = traverse(a, b)
            .into_iter()
            .rev()
            .map(|c| Crossing { hex: c.hex, t_in: c.t_out.one_minus(), t_out: c.t_in.one_minus() })
            .collect();
        prop_assert_eq!(forward, traverse(b, a));
    }

    #[test]
    fn rotating_the_segment_rotates_the_crossings(a in point(), b in point(), k in rotation()) {
        let expected: Vec<Crossing> = traverse(a, b)
            .into_iter()
            .map(|c| Crossing { hex: c.hex.rotate(k), ..c })
            .collect();
        prop_assert_eq!(expected, traverse(rotate_point(a, k), rotate_point(b, k)));
    }

    #[test]
    fn interior_endpoints_start_and_end_the_traversal(a in point(), b in point()) {
        prop_assume!(a != b);
        let crossings = traverse(a, b);
        for inside in traverse(a, a) {
            prop_assert_eq!(crossings.first().map(|c| c.hex), Some(inside.hex));
        }
        for inside in traverse(b, b) {
            prop_assert_eq!(crossings.last().map(|c| c.hex), Some(inside.hex));
        }
    }

    #[test]
    fn inset_crossing_lies_within_the_full_crossing(a in point(), b in point()) {
        for c in traverse(a, b) {
            if let Some((t_in, t_out)) = clip_inset(c.hex, a, b) {
                prop_assert!(c.t_in <= t_in && t_in < t_out && t_out <= c.t_out);
            }
        }
    }

    #[test]
    fn traversal_is_deterministic(a in point(), b in point()) {
        prop_assert_eq!(traverse(a, b), traverse(a, b));
    }
}

#[test]
fn frac_normalizes_and_orders() {
    assert_eq!(frac(2, -4), frac(-1, 2));
    assert_eq!(frac(0, -7), Frac::ZERO);
    assert!(frac(1, 3) < frac(1, 2));
    assert_eq!(frac(7, 2).floor(), 3);
    assert_eq!(frac(-7, 2).floor(), -4);
    assert_eq!(frac(-7, 2).ceil(), -3);
    assert_eq!(frac(1, 4).one_minus(), frac(3, 4));
}

#[test]
fn center_to_center_along_each_axis() {
    for (i, direction) in DIRECTIONS.iter().enumerate() {
        let start = Hex::new(2, -3);
        let end = start.add(direction.scale(3));
        let crossings = traverse(Point::center(start), Point::center(end));
        let expected: Vec<Hex> = (0..=3).map(|k| start.add(direction.scale(k))).collect();
        assert_eq!(hexes(&crossings), expected, "direction {i}");
        assert_eq!(crossings[0].t_in, Frac::ZERO);
        assert_eq!(crossings[3].t_out, Frac::ONE);
        for pair in crossings.windows(2) {
            assert_eq!(pair[0].t_out, pair[1].t_in);
        }
    }
}

#[test]
fn east_traversal_has_exact_parameters() {
    let crossings = traverse(Point::new(0, 0), Point::new(3 * LATTICE, 0));
    assert_eq!(
        crossings,
        vec![
            crossing(0, 0, frac(0, 1), frac(1, 6)),
            crossing(1, 0, frac(1, 6), frac(1, 2)),
            crossing(2, 0, frac(1, 2), frac(5, 6)),
            crossing(3, 0, frac(5, 6), frac(1, 1)),
        ]
    );
}

#[test]
fn segment_along_a_shared_edge_crosses_neither_hex() {
    // The edge between (0, 0) and (1, 0) runs from corner (5, 5) to corner (10, -5).
    assert!(traverse(Point::new(5, 5), Point::new(10, -5)).is_empty());
    assert!(traverse(Point::new(10, -5), Point::new(5, 5)).is_empty());
}

#[test]
fn edge_run_then_interior_counts_only_the_interior() {
    // Runs along the edge to corner (5, 5), then on into the center of (0, 1).
    let crossings = traverse(Point::new(10, -5), Point::new(0, 15));
    assert_eq!(crossings, vec![crossing(0, 1, frac(1, 2), frac(1, 1))]);
}

#[test]
fn vertex_grazing_does_not_count() {
    // Passes through corner (5, 5) of (0, 0) while touching only (1, 0) and (0, 1).
    let crossings = traverse(Point::new(10, 0), Point::new(0, 10));
    assert_eq!(
        crossings,
        vec![
            crossing(1, 0, frac(0, 1), frac(1, 2)),
            crossing(0, 1, frac(1, 2), frac(1, 1)),
        ]
    );
}

#[test]
fn zero_length_segment_reports_its_hex() {
    assert_eq!(
        traverse(Point::new(15, 0), Point::new(15, 0)),
        vec![crossing(1, 0, frac(0, 1), frac(1, 1))]
    );
    assert!(traverse(Point::new(5, 5), Point::new(5, 5)).is_empty());
}

#[test]
fn inset_hex_is_smaller_than_the_hex() {
    let origin = Hex::ORIGIN;
    let (a, b) = (Point::new(-30, 0), Point::new(30, 0));
    // The full hex spans q in (-7.5, 7.5); the inset hex spans (-6, 6).
    let full = traverse(a, b)
        .into_iter()
        .find(|c| c.hex == origin)
        .unwrap();
    assert_eq!((full.t_in, full.t_out), (frac(3, 8), frac(5, 8)));
    assert_eq!(clip_inset(origin, a, b), Some((frac(2, 5), frac(3, 5))));
    // A segment between the inset and full boundaries misses the inset hex.
    assert_eq!(
        clip_inset(origin, Point::new(9, -20), Point::new(9, 20)),
        None
    );
    let crossed = traverse(Point::new(9, -20), Point::new(9, 20));
    assert!(crossed.iter().any(|c| c.hex == origin));
}

#[test]
fn inset_corners_are_integer_points() {
    // Inset corner of (0, 0) toward (5, 5) is (4, 4): touching it is not crossing.
    assert_eq!(
        clip_inset(Hex::ORIGIN, Point::new(4, 4), Point::new(4, 4)),
        None
    );
    assert!(clip_inset(Hex::ORIGIN, Point::new(3, 3), Point::new(3, 3)).is_some());
}
