//! Placing footprints on small boards: surfaces, level spread and bridges.

use std::collections::BTreeMap;

use strata_kernel::{DefId, Hex, Rotation, Shape, Space, Sym};

use super::{FootprintDef, MoveMode};
use crate::board::{BoardDefs, CompiledBoard, compile};
use crate::map::{MapConstraints, MapDef, PlacedPiece};
use crate::terrain::{FeatureDef, TileDef};
use crate::vocab::{SourceLoc, Text};

struct Defs(BTreeMap<Sym, TileDef>);

impl BoardDefs for Defs {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        self.0.get(name)
    }
    fn feature(&self, _: &Sym) -> Option<&FeatureDef> {
        None
    }
}

/// One grass tile per `(q, r, level)`.
fn board(pieces: &[(i32, i32, u8)]) -> CompiledBoard {
    let grass = TileDef {
        id: DefId(0),
        name: Text::default(),
        terrain: DefId(1),
        thickness: 1,
        shape: Shape::new([Hex::new(0, 0)]),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    let pieces = pieces.iter().map(|&(q, r, level)| PlacedPiece {
        piece: "grass".into(),
        at: Hex::new(q, r),
        rot: Rotation::default(),
        level,
    });
    let map = MapDef {
        id: DefId(0),
        name: Text::default(),
        authors: Vec::new(),
        orientation: "pointy".into(),
        pieces: pieces.collect(),
        features: Vec::new(),
        zones: Default::default(),
        relic_spots: Default::default(),
        constraints: MapConstraints::default(),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    compile(&map, &Defs(BTreeMap::from([("grass".into(), grass)]))).unwrap()
}

fn two(max_spread: u8) -> FootprintDef {
    FootprintDef {
        id: DefId(0),
        name: Text::default(),
        cells: Shape::new([Hex::new(0, 0), Hex::new(1, 0)]),
        move_mode: MoveMode::Snake,
        max_spread,
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    }
}

fn spaces(def: &FootprintDef, board: &CompiledBoard, at: Space, o: u8) -> Option<Vec<Space>> {
    let found = def.spaces(board, at, Rotation::new(o).unwrap())?;
    Some(found.into_iter().map(|(space, _)| space).collect())
}

#[test]
fn the_cells_turn_with_the_rotation() {
    let def = two(1);
    let at = |o: u8| def.cells_at(Hex::new(5, 5), Rotation::new(o).unwrap());
    assert_eq!(at(0), [Hex::new(5, 5), Hex::new(6, 5)]);
    assert_eq!(at(1), [Hex::new(5, 5), Hex::new(5, 6)]);
    assert_eq!(at(3), [Hex::new(5, 5), Hex::new(4, 5)]);
}

#[test]
fn a_cell_over_empty_space_has_no_placement() {
    let board = board(&[(0, 0, 0), (1, 0, 0)]);
    let def = two(1);
    let at = Space::new(0, 0, 1);
    assert_eq!(
        spaces(&def, &board, at, 0),
        Some(vec![at, Space::new(1, 0, 1)])
    );
    // Facing west, the rear cell hangs over (-1, 0), where there is nothing.
    assert_eq!(spaces(&def, &board, at, 3), None);
}

#[test]
fn the_level_spread_is_limited() {
    let board = board(&[(0, 0, 0), (1, 0, 1), (2, 0, 3)]);
    let flat = two(0);
    let one = two(1);
    let at = Space::new(0, 0, 1);
    assert_eq!(spaces(&flat, &board, at, 0), None);
    assert_eq!(
        spaces(&one, &board, at, 0),
        Some(vec![at, Space::new(1, 0, 2)])
    );
    // From (1, 0) at level 2 the next column stands at 4: a spread of two.
    assert_eq!(spaces(&one, &board, Space::new(1, 0, 2), 0), None);
    assert!(spaces(&two(2), &board, Space::new(1, 0, 2), 0).is_some());
}

#[test]
fn a_cell_stands_on_the_surface_nearest_the_anchor() {
    // A bridge over (1, 0): ground at 1 and a deck at 5.
    let board = board(&[(0, 0, 4), (1, 0, 0), (1, 0, 4), (2, 0, 0)]);
    let def = two(1);
    let deck = spaces(&def, &board, Space::new(0, 0, 5), 0);
    assert_eq!(deck, Some(vec![Space::new(0, 0, 5), Space::new(1, 0, 5)]));
    let under = spaces(&def, &board, Space::new(2, 0, 1), 3);
    assert_eq!(under, Some(vec![Space::new(2, 0, 1), Space::new(1, 0, 1)]));
}

#[test]
fn no_placement_ever_leaves_a_cell_without_a_surface() {
    // Every anchor and rotation over a ragged little board.
    let pieces = [
        (0, 0, 0),
        (1, 0, 1),
        (2, 0, 0),
        (0, 1, 2),
        (1, 1, 0),
        (2, -1, 3),
    ];
    let board = board(&pieces);
    for spread in 0..4 {
        let def = two(spread);
        for &(q, r, level) in &pieces {
            let at = Space::new(q, r, u32::from(level) + 1);
            for o in 0..6 {
                let Some(found) = def.spaces(&board, at, Rotation::new(o).unwrap()) else {
                    continue;
                };
                for (space, surface) in &found {
                    assert!(board.surfaces(space.hex()).contains(surface));
                    assert_eq!(space.z, surface.z);
                }
                let levels: Vec<_> = found.iter().map(|(s, _)| s.z).collect();
                let wide = levels.iter().max().unwrap() - levels.iter().min().unwrap();
                assert!(wide <= u32::from(spread));
            }
        }
    }
}
