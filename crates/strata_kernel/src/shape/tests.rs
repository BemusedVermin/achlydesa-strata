use super::*;
use crate::shape::common;

fn named() -> Vec<(Shape, usize)> {
    vec![
        (common::one(), 1),
        (common::two_line(), 2),
        (common::three_line(), 3),
        (common::triangle(), 3),
        (common::flower(), 7),
    ]
}

#[test]
fn named_shapes_have_expected_size_and_are_connected() {
    for (shape, len) in named() {
        assert_eq!(shape.len(), len);
        assert!(shape.is_connected());
    }
}

#[test]
fn rotation_and_translation_keep_count_and_connectivity() {
    for (shape, len) in named() {
        for k in 0..6 {
            let moved = shape
                .rotated(Rotation::from_steps(k))
                .translated(Hex::new(-4, 7));
            assert_eq!(moved.len(), len);
            assert!(moved.is_connected());
        }
    }
}

#[test]
fn six_rotations_restore_shape() {
    for (shape, _) in named() {
        let mut turned = shape.clone();
        for _ in 0..6 {
            turned = turned.rotated(Rotation::from_steps(1));
        }
        assert_eq!(turned, shape);
    }
}

#[test]
fn gap_is_disconnected_and_empty_is_not_connected() {
    assert!(!Shape::new([Hex::ORIGIN, Hex::new(2, 0)]).is_connected());
    assert!(!Shape::default().is_connected());
}

#[test]
fn new_sorts_and_dedups() {
    let shape = Shape::new([Hex::new(1, 0), Hex::ORIGIN, Hex::new(1, 0)]);
    assert_eq!(shape.cells(), [Hex::ORIGIN, Hex::new(1, 0)]);
}

#[test]
fn serde_round_trips() {
    let shape = common::flower();
    let json = serde_json::to_string(&shape).unwrap();
    assert_eq!(serde_json::from_str::<Shape>(&json).unwrap(), shape);
}
