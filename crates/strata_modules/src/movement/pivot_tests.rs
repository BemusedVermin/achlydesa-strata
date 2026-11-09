//! Turning in place: two-cell figures pivot about their anchor at no cost, one 60-degree turn at
//! a time, through placements that must each be legal (#187).

use std::collections::BTreeMap;

use proptest::prelude::*;
use strata_kernel::{Hex, Rotation, Space};
use strata_model::{FigureId, MoveMode};
use strata_rules::Reach;

use super::body::Body;
use super::footprint_tests::{
    Figure, LIMITS, Scene, TWO, board, flat, footprint, pivots, ragged, run,
};
use super::search::Cache;

/// The anchor every scene starts on.
const START: Space = Space::new(0, 0, 1);

/// The routes that turn the figure where it stands, by rotation.
fn in_place(reach: &[Reach]) -> BTreeMap<u8, &Reach> {
    let here = reach.iter().filter(|r| r.space == START);
    here.map(|r| (r.rotation.steps(), r)).collect()
}

/// The rotations the figure can turn to where it stands.
fn turned(reach: &[Reach]) -> Vec<u8> {
    in_place(reach).into_keys().collect()
}

/// A scene with a two-cell figure in `mode` on flat ground, no allowance, rear cell east.
fn two_cells(mode: MoveMode) -> Scene {
    let mut scene = Scene::new(flat(3), Some(footprint(&TWO, mode, 1)));
    scene.allowance = 0;
    scene
}

/// A one-cell figure standing on `(q, r)` in the way.
fn blocker(q: i32, r: i32) -> Figure {
    Figure {
        at: Space::new(q, r, 1),
        rotation: Rotation::default(),
        footprint: None,
    }
}

#[test]
fn a_two_cell_figure_turns_in_place_at_no_cost() {
    for mode in [MoveMode::Rigid, MoveMode::Snake] {
        let reach = run(&two_cells(mode));
        // With no allowance at all it can still turn to every other rotation.
        assert_eq!(turned(&reach), [1, 2, 3, 4, 5], "{mode:?}");
        for (o, r) in in_place(&reach) {
            assert_eq!(r.cost, 0, "{r:?}");
            assert!(!r.falls && !r.stops_here && !r.occupied, "{r:?}");
            // One pivot per 60 degrees, the short way round.
            let turns = usize::from(o.min(6 - o));
            assert_eq!(r.path, vec![START; turns], "{r:?}");
            assert_eq!(pivots(r, START), turns);
            // The walk holds the body after every pivot: the anchor stays, the rear swings.
            assert_eq!(r.walk.len(), turns + 1);
            assert!(r.walk.iter().all(|cells| cells[0] == START));
            let rear = START.hex().add(Hex::new(1, 0).rotate(r.rotation));
            assert_eq!(r.cells, [START, Space::at(rear, 1)]);
        }
    }
}

#[test]
fn a_two_cell_figure_turns_and_then_moves() {
    // A rigid shape steps only in the rotation it has, so pivots let it reach a space in any.
    let mut scene = two_cells(MoveMode::Rigid);
    scene.allowance = 1;
    let reach = run(&scene);
    let se = reach.iter().filter(|r| r.space == Space::new(0, 1, 1));
    let se: Vec<_> = se.collect();
    assert_eq!(se.len(), 6);
    assert!(se.iter().all(|r| r.cost == 1), "{se:?}");
    // Facing west, it turned first: the route that ends in a step wins the tie.
    let west = se.iter().find(|r| r.rotation.steps() == 3).unwrap();
    let path = [START, START, START, Space::new(0, 1, 1)];
    assert_eq!(west.path, path);
}

#[test]
fn a_turn_sweeps_past_a_figure_the_long_way_round() {
    let mut scene = two_cells(MoveMode::Rigid);
    // A figure south-east of the anchor blocks the first clockwise turn.
    scene.figures.push(blocker(0, 1));
    let reach = run(&scene);
    assert_eq!(turned(&reach), [2, 3, 4, 5]);
    let o2 = in_place(&reach)[&2];
    assert_eq!(pivots(o2, START), 4, "counterclockwise, through 5, 4 and 3");
    // With the north-east neighbour blocked too, the rear cannot swing either way.
    scene.figures.push(blocker(1, -1));
    assert!(turned(&run(&scene)).is_empty());
}

#[test]
fn a_turn_needs_a_surface_for_every_rotation_it_sweeps() {
    // A line of three columns: the rear may stand west of the anchor too, but the half-turn
    // there would sweep over empty columns.
    let line = [(-1, 0), (0, 0), (1, 0)].map(|(q, r)| (q, r, 0, false));
    let mut scene = Scene::new(board(&line), Some(footprint(&TWO, MoveMode::Rigid, 1)));
    scene.allowance = 0;
    assert!(turned(&run(&scene)).is_empty());
}

#[test]
fn a_turn_respects_can_enter_climb_and_stop_terrain() {
    let mut scene = two_cells(MoveMode::Rigid);
    scene.barred.insert(Space::new(0, 1, 1));
    assert_eq!(
        turned(&run(&scene)),
        [2, 3, 4, 5],
        "`can_enter` vetoes (0, 1)"
    );

    // The column south-east of the anchor two levels up: within the spread, beyond `max_climb`.
    let mut pieces: Vec<_> = Hex::ORIGIN
        .spiral(2)
        .iter()
        .map(|h| (h.q, h.r, 0, false))
        .collect();
    pieces.extend([(0, 1, 1, false), (0, 1, 2, false)]);
    let mut scene = Scene::new(board(&pieces), Some(footprint(&TWO, MoveMode::Rigid, 2)));
    scene.allowance = 0;
    assert_eq!(
        turned(&run(&scene)),
        [2, 3, 4, 5],
        "the rear would climb two levels"
    );
    // One level up is a legal climb.
    pieces.pop();
    scene.board = board(&pieces);
    let reach = run(&scene);
    assert_eq!(in_place(&reach)[&1].cells, [START, Space::new(0, 1, 2)]);

    // Mud south-east of the anchor: turning the rear onto it ends the move there.
    let mut pieces: Vec<_> = Hex::ORIGIN
        .spiral(2)
        .iter()
        .map(|h| (h.q, h.r, 0, false))
        .collect();
    pieces.retain(|p| (p.0, p.1) != (0, 1));
    pieces.push((0, 1, 0, true));
    let mut scene = Scene::new(board(&pieces), Some(footprint(&TWO, MoveMode::Rigid, 1)));
    scene.allowance = 0;
    let reach = run(&scene);
    assert!(in_place(&reach)[&1].stops_here);
    assert_eq!(
        pivots(in_place(&reach)[&2], START),
        4,
        "not through the mud"
    );
}

#[test]
fn a_figure_already_on_stop_terrain_may_still_turn_and_move() {
    // The anchor stands on mud: a pivot does not enter it again, so it does not end the move.
    let mut pieces: Vec<_> = Hex::ORIGIN
        .spiral(2)
        .iter()
        .map(|h| (h.q, h.r, 0, false))
        .collect();
    pieces.retain(|p| (p.0, p.1) != (0, 0));
    pieces.push((0, 0, 0, true));
    let mut scene = Scene::new(board(&pieces), Some(footprint(&TWO, MoveMode::Rigid, 1)));
    scene.allowance = 1;
    let reach = run(&scene);
    assert!(!in_place(&reach)[&3].stops_here);
    // Some route turns where it stands and then steps off the mud.
    let turned_then_stepped = |r: &&Reach| r.space != START && r.path[0] == START;
    assert!(reach.iter().any(|r| turned_then_stepped(&r)), "{reach:?}");
}

#[test]
fn turning_the_rear_away_from_an_enemy_provokes_a_reprisal() {
    // The enemy on (2, -1) is next to the rear cell on (1, 0) only.
    let mut scene = two_cells(MoveMode::Rigid);
    scene.enemies = vec![Hex::new(2, -1)];
    let reach = run(&scene);
    let turns = in_place(&reach);
    assert!(turns[&1].provokes_reprisal, "the rear swings to (0, 1)");
    assert!(
        !turns[&5].provokes_reprisal,
        "the rear on (1, -1) stays next to it"
    );
    assert!(
        turns[&4].provokes_reprisal,
        "past (1, -1) the rear leaves it"
    );
}

#[test]
fn a_three_cell_shape_never_pivots() {
    let three = footprint(&[(0, 0), (1, 0), (0, 1)], MoveMode::Rigid, 1);
    let mut scene = Scene::new(flat(3), Some(three.clone()));
    scene.allowance = 2;
    let reach = run(&scene);
    assert!(!reach.is_empty());
    assert!(reach.iter().all(|r| r.rotation == Rotation::default()));
    assert!(reach.iter().all(|r| pivots(r, START) == 0));
    // Asked directly, the body offers no pivot.
    let mut cache = Cache::new(&scene, FigureId(0));
    let before = scene_cells(&scene);
    let offered =
        Body::Shaped(&three).pivots(&mut cache, LIMITS, 1, (START, Rotation::default()), &before);
    assert!(offered.unwrap().is_empty());
    // Nor does a one-cell figure, which has nothing to turn.
    let offered = Body::One.pivots(
        &mut cache,
        LIMITS,
        1,
        (START, Rotation::default()),
        &[START],
    );
    assert!(offered.unwrap().is_empty());
}

/// The cells the scene's mover covers.
fn scene_cells(scene: &Scene) -> Vec<Space> {
    let def = scene.figures[0].footprint.as_ref().unwrap();
    let spaces = def.spaces_where(&scene.board, START, Rotation::default(), |_, _| true);
    spaces.unwrap().into_iter().map(|(s, _)| s).collect()
}

proptest! {
    #[test]
    fn pivots_never_change_the_cost(
        levels in proptest::collection::vec(0u8..3, 37),
        holes in proptest::collection::btree_set(1usize..37, 0..10),
        mud in proptest::collection::btree_set(1usize..37, 0..4),
        snake in any::<bool>(),
        rotation in 0u8..6,
        allowance in 0i64..4,
    ) {
        let board = ragged(&levels, &holes, &mud, &BTreeMap::new());
        let mode = if snake { MoveMode::Snake } else { MoveMode::Rigid };
        let mut scene = Scene::new(board, Some(footprint(&TWO, mode, 2)));
        let from = Space::new(0, 0, u32::from(levels[0]) + 1);
        scene.figures[0].at = from;
        scene.figures[0].rotation = Rotation::new(rotation).unwrap();
        scene.allowance = allowance;
        scene.enemies = vec![Hex::new(2, -1)];
        let reach = run(&scene);
        let by_key: BTreeMap<_, _> = reach.iter().map(|r| ((r.space, r.rotation), r)).collect();
        for r in &reach {
            // Every step here costs one, so a route costs exactly its steps: pivots are free.
            let steps = r.path.len() - pivots(r, from);
            prop_assert_eq!(r.cost, steps as i64, "{:?}", r);
            prop_assert!(r.cost <= allowance);
            prop_assert_eq!(r.walk.len(), r.path.len() + 1);
            // A route that ends in a pivot costs what the node it turned from costs.
            let before = r.path.iter().rev().nth(1).copied().unwrap_or(from);
            if before == r.space && r.path.len() > 1 {
                let previous = &r.walk[r.walk.len() - 2];
                let turned_from = by_key.values().find(|p| p.space == r.space && &p.cells == previous);
                if let Some(p) = turned_from {
                    prop_assert_eq!(p.cost, r.cost);
                }
            }
        }
        // The same query twice gives the same answer.
        prop_assert_eq!(run(&scene), reach);
    }
}
