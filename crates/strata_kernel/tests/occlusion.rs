//! Tests for 3D occlusion (T-007).

use proptest::prelude::*;
use strata_kernel::{
    Blocker, BlockerFootprint, Hex, Point, Point3, Rotation, SUB_LEVELS, Seg3, blocked, clear_count,
};

const FULL: BlockerFootprint = BlockerFootprint::FullHex;
const INSET: BlockerFootprint = BlockerFootprint::InsetHex;

fn blocker(footprint: BlockerFootprint, z0: i64, z1: i64) -> Blocker {
    Blocker { footprint, z0, z1 }
}

fn p3(q: i64, r: i64, z: i64) -> Point3 {
    Point3::new(Point::new(q, r), z)
}

fn center(q: i32, r: i32, z: i64) -> Point3 {
    Point3::new(Point::center(Hex::new(q, r)), z)
}

/// A scene with one blocker list at one hex.
fn scene<'a>(hex: Hex, list: &'a [Blocker]) -> impl Fn(Hex) -> &'a [Blocker] {
    move |h| if h == hex { list } else { &[] }
}

const ENDS: [Hex; 2] = [Hex::new(0, 0), Hex::new(2, 0)];

#[test]
fn wall_blocks_low_samples_but_not_high_ones() {
    let wall = [blocker(FULL, 0, 3 * SUB_LEVELS)];
    let at = scene(Hex::new(1, 0), &wall);
    let low = Seg3::new(center(0, 0, 6), center(2, 0, 6));
    let high = Seg3::new(center(0, 0, 40), center(2, 0, 40));
    assert!(blocked(low, &at, &ENDS));
    assert!(!blocked(high, &at, &ENDS));
}

#[test]
fn clear_count_counts_unblocked_samples_and_returns_witnesses() {
    let wall = [blocker(FULL, 0, 3 * SUB_LEVELS)];
    let at = scene(Hex::new(1, 0), &wall);
    let targets = [center(2, 0, 6), center(2, 0, 40), center(2, 0, 50)];
    let origin = center(0, 0, 40);
    let result = clear_count(origin, &targets, &at, &ENDS);
    assert_eq!(result.clear, 2);
    assert_eq!(
        result.witnesses,
        vec![Seg3::new(origin, targets[1]), Seg3::new(origin, targets[2]),]
    );
    let none = clear_count(center(0, 0, 4), &targets[..1], &at, &ENDS);
    assert_eq!(none.clear, 0);
    assert!(none.witnesses.is_empty());
}

#[test]
fn bridge_deck_blocks_through_but_not_under() {
    let deck = [blocker(FULL, 48, 60)];
    let at = scene(Hex::new(1, 0), &deck);
    assert!(blocked(
        Seg3::new(center(0, 0, 54), center(2, 0, 54)),
        &at,
        &ENDS
    ));
    assert!(blocked(
        Seg3::new(center(0, 0, 0), center(2, 0, 100)),
        &at,
        &ENDS
    ));
    assert!(!blocked(
        Seg3::new(center(0, 0, 12), center(2, 0, 12)),
        &at,
        &ENDS
    ));
}

#[test]
fn touching_top_or_bottom_does_not_block() {
    let solid = [blocker(FULL, 12, 24)];
    let at = scene(Hex::new(1, 0), &solid);
    assert!(!blocked(
        Seg3::new(center(0, 0, 24), center(2, 0, 24)),
        &at,
        &ENDS
    ));
    assert!(!blocked(
        Seg3::new(center(0, 0, 12), center(2, 0, 12)),
        &at,
        &ENDS
    ));
    // Sloping so that it meets the top plane only at the column's far boundary.
    // Column (1,0) spans t in (1/4, 3/4) of this segment, and the height is 24 at t = 3/4.
    let grazing = Seg3::new(center(0, 0, 60), center(2, 0, 12));
    assert!(!blocked(grazing, &at, &ENDS));
    let piercing = Seg3::new(center(0, 0, 60), center(2, 0, 0));
    assert!(blocked(piercing, &at, &ENDS));
}

#[test]
fn touching_a_side_or_corner_does_not_block() {
    let solid = [blocker(FULL, 0, 100)];
    let at = scene(Hex::new(0, 0), &solid);
    // Along the edge shared by (0,0) and (1,0): where 2q + r = 15.
    let along_edge = Seg3::new(p3(15, -15, 6), p3(0, 15, 6));
    assert!(!blocked(along_edge, &at, &[]));
    // Reaches the corner (10, -5) of (0,0) from outside and stops there.
    let to_corner = Seg3::new(p3(20, -10, 6), p3(10, -5, 6));
    assert!(!blocked(to_corner, &at, &[]));
}

#[test]
fn inset_blocker_leaves_the_rim_open() {
    let figure = [blocker(INSET, 0, 24)];
    let at = scene(Hex::new(0, 0), &figure);
    let rim = Seg3::new(p3(-30, 9, 6), p3(30, 9, 6));
    assert!(!blocked(rim, &at, &[]));
    let middle = Seg3::new(p3(-30, 0, 6), p3(30, 0, 6));
    assert!(blocked(middle, &at, &[]));
    let full = [blocker(FULL, 0, 24)];
    assert!(blocked(rim, scene(Hex::new(0, 0), &full), &[]));
}

#[test]
fn ignored_columns_are_skipped() {
    let wall = [blocker(FULL, 0, 100)];
    let at = scene(Hex::new(0, 0), &wall);
    let seg = Seg3::new(center(0, 0, 6), center(2, 0, 6));
    assert!(blocked(seg, &at, &[]));
    assert!(!blocked(seg, &at, &[Hex::new(0, 0)]));
}

fn rotate_p3(p: Point3, k: Rotation) -> Point3 {
    let h = Hex::new(p.xy.q as i32, p.xy.r as i32).rotate(k);
    p3(h.q.into(), h.r.into(), p.z)
}

fn point3() -> impl Strategy<Value = Point3> {
    (-60i64..60, -60i64..60, 0i64..60).prop_map(|(q, r, z)| p3(q, r, z))
}

fn placed() -> impl Strategy<Value = Vec<(Hex, Blocker)>> {
    let one = ((-4i32..4, -4i32..4), any::<bool>(), 0i64..48, 1i64..24).prop_map(
        |((q, r), inset, z0, h)| {
            let footprint = if inset { INSET } else { FULL };
            (Hex::new(q, r), blocker(footprint, z0, z0 + h))
        },
    );
    prop::collection::vec(one, 0..8)
}

/// Groups blockers by hex so a lookup can hand out slices.
fn group(placed: &[(Hex, Blocker)]) -> Vec<(Hex, Vec<Blocker>)> {
    let mut groups: Vec<(Hex, Vec<Blocker>)> = Vec::new();
    for (hex, blocker) in placed {
        match groups.iter_mut().find(|(h, _)| h == hex) {
            Some((_, list)) => list.push(*blocker),
            None => groups.push((*hex, vec![*blocker])),
        }
    }
    groups
}

fn lookup(groups: &[(Hex, Vec<Blocker>)], hex: Hex) -> &[Blocker] {
    groups
        .iter()
        .find(|(h, _)| *h == hex)
        .map_or(&[], |(_, list)| list)
}

proptest! {
    #[test]
    fn invariant_under_rotation(a in point3(), b in point3(), placed in placed(), k in 0u8..6) {
        let k = Rotation::new(k).unwrap();
        let rotated: Vec<(Hex, Blocker)> =
            placed.iter().map(|(h, bl)| (h.rotate(k), *bl)).collect();
        let (groups, rotated_groups) = (group(&placed), group(&rotated));
        let before = blocked(Seg3::new(a, b), |h| lookup(&groups, h), &[]);
        let after = blocked(
            Seg3::new(rotate_p3(a, k), rotate_p3(b, k)),
            |h| lookup(&rotated_groups, h),
            &[],
        );
        prop_assert_eq!(before, after);
    }

    #[test]
    fn invariant_under_swapping_endpoints(a in point3(), b in point3(), placed in placed()) {
        let groups = group(&placed);
        let seg = Seg3::new(a, b);
        prop_assert_eq!(
            blocked(seg, |h| lookup(&groups, h), &[]),
            blocked(seg.reversed(), |h| lookup(&groups, h), &[]),
        );
    }
}
