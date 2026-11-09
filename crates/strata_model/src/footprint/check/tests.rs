//! Footprint shapes the engine accepts and refuses.

use strata_kernel::{DefId, Hex, Rotation, Shape};

use super::super::{FootprintDef, MoveMode};
use super::{FootprintError, MAX_SNAKE_CELLS};
use crate::vocab::{SourceLoc, Text};

fn footprint(cells: &[(i32, i32)], move_mode: MoveMode) -> FootprintDef {
    FootprintDef {
        id: DefId(0),
        name: Text::default(),
        cells: Shape::new(cells.iter().map(|&(q, r)| Hex::new(q, r))),
        move_mode,
        max_spread: 1,
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    }
}

const LINE: [(i32, i32); 3] = [(0, 0), (1, 0), (2, 0)];
const BENT: [(i32, i32); 3] = [(0, 0), (1, 0), (1, 1)];

#[test]
fn one_and_two_cell_shapes_are_accepted_in_either_mode() {
    for mode in [MoveMode::Rigid, MoveMode::Snake] {
        assert_eq!(footprint(&[(0, 0)], mode).check(), Ok(()));
        assert_eq!(footprint(&[(0, 0), (1, 0)], mode).check(), Ok(()));
        assert_eq!(footprint(&[(0, 0), (-1, 1)], mode).check(), Ok(()));
    }
}

#[test]
fn larger_connected_shapes_are_accepted_when_rigid() {
    for cells in [&LINE[..], &BENT[..], &[(0, 0), (1, 0), (0, 1), (-1, 0)]] {
        assert_eq!(footprint(cells, MoveMode::Rigid).check(), Ok(()));
    }
    // The anchor need not be an end of the shape.
    let middle = [(-1, 0), (0, 0), (1, 0)];
    assert_eq!(footprint(&middle, MoveMode::Rigid).check(), Ok(()));
}

#[test]
fn an_empty_shape_is_refused() {
    let err = footprint(&[], MoveMode::Rigid).check().unwrap_err();
    assert_eq!(err, FootprintError::Empty);
    assert_eq!(err.field(), "cells");
}

#[test]
fn a_shape_without_the_anchor_is_refused() {
    let err = footprint(&[(1, 0), (2, 0)], MoveMode::Rigid).check();
    assert_eq!(err, Err(FootprintError::NoAnchor));
    assert!(err.unwrap_err().to_string().contains("q = 0, r = 0"));
}

#[test]
fn a_disconnected_shape_is_refused_naming_the_stray_cell() {
    let err = footprint(&[(0, 0), (2, 0)], MoveMode::Rigid).check();
    let stray = Hex::new(2, 0);
    assert_eq!(err, Err(FootprintError::Disconnected { cell: stray }));
    // Two cells joined to each other but not to the anchor are still disconnected.
    let island = footprint(&[(0, 0), (3, 0), (4, 0)], MoveMode::Rigid).check();
    let message = island.unwrap_err().to_string();
    assert!(message.contains("q = 3, r = 0"), "{message}");
}

#[test]
fn a_snake_longer_than_two_cells_is_refused_and_told_to_be_rigid() {
    for cells in [&LINE[..], &BENT[..]] {
        let err = footprint(cells, MoveMode::Snake).check().unwrap_err();
        assert_eq!(err, FootprintError::LongSnake { cells: 3 });
        assert_eq!(err.field(), "move");
        assert!(err.to_string().contains("rigid"), "{err}");
    }
    assert_eq!(MAX_SNAKE_CELLS, 2);
}

#[test]
fn a_check_does_not_depend_on_the_rotation() {
    let shapes: [&[(i32, i32)]; 6] = [&LINE, &BENT, &[(0, 0), (2, 0)], &[(1, 0)], &[], &[(0, 0)]];
    for cells in shapes {
        for mode in [MoveMode::Rigid, MoveMode::Snake] {
            let def = footprint(cells, mode);
            for turn in 0..6 {
                let mut turned = def.clone();
                turned.cells = def.cells.rotated(Rotation::new(turn).unwrap());
                assert_eq!(
                    def.check().is_ok(),
                    turned.check().is_ok(),
                    "{cells:?} {turn}"
                );
            }
        }
    }
}
