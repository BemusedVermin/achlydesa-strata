use strata_kernel::{Hex, Rotation};

use super::*;
use crate::sprite::direction;

const TWO: [Hex; 2] = [Hex::ORIGIN, Hex::new(1, 0)];
const WEDGE: [Hex; 4] = [
    Hex::ORIGIN,
    Hex::new(-1, 0),
    Hex::new(-1, 1),
    Hex::new(0, 1),
];
const LINE: [Hex; 3] = [Hex::ORIGIN, Hex::new(1, 0), Hex::new(-1, 0)];

fn turn(steps: i32) -> Rotation {
    Rotation::from_steps(steps)
}

#[test]
fn a_one_cell_figure_faces_its_rotation() {
    for r in 0..6 {
        assert_eq!(facing(&[Hex::ORIGIN], turn(r)), turn(r));
        assert_eq!(facing(&[], turn(r)), turn(r));
    }
}

#[test]
fn a_two_cell_figure_faces_away_from_its_rear() {
    for r in 0..6 {
        assert_eq!(facing(&TWO, turn(r)), turn(r + 3));
    }
}

#[test]
fn a_snake_walking_east_shows_its_east_facing_frame() {
    // Movement turns the two-cell base so its rear trails: stepping east (direction 0) leaves the
    // figure at rotation 3, with its rear on the west cell it came from.
    let rotation = turn(3);
    let cells: Vec<Hex> = TWO.iter().map(|c| c.rotate(rotation)).collect();
    assert_eq!(cells[1], Hex::ORIGIN.neighbor(3), "the rear is to the west");
    let east = facing(&TWO, rotation);
    assert_eq!(east, turn(0));
    assert_eq!(direction(east, 0), 0, "the east frame at camera step 0");
}

#[test]
fn a_larger_figure_faces_from_its_other_cells_toward_its_anchor() {
    // Three rear cells behind the anchor: their offsets sum to (2, -2), toward NE.
    assert_eq!(facing(&WEDGE, turn(0)), turn(5));
    // With the anchor in the middle the offsets cancel, so it faces away from its first cell.
    assert_eq!(facing(&LINE, turn(0)), turn(3));
    // A cell off every direction faces the nearest one, the lower on a tie.
    let knight = [Hex::ORIGIN, Hex::new(2, -1)];
    assert_eq!(facing(&knight, turn(0)), turn(2));
}

#[test]
fn facing_turns_with_the_figure() {
    for shape in [&TWO[..], &WEDGE[..], &LINE[..]] {
        let base = i32::from(facing(shape, turn(0)).steps());
        for r in 0..6 {
            let rotated: Vec<Hex> = shape.iter().map(|c| c.rotate(turn(r))).collect();
            assert_eq!(facing(shape, turn(r)), turn(base + r));
            assert_eq!(facing(&rotated, turn(0)), facing(shape, turn(r)));
        }
    }
}
