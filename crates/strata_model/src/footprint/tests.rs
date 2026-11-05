//! Where a footprint stands: anchors, cell levels, spread and rotation.

use std::collections::BTreeSet;

use proptest::prelude::*;
use strata_kernel::{DefId, Hex, Rotation, Shape, Space, Sym};

use super::{FootprintDef, FootprintError, MoveMode};
use crate::board::{BoardDefs, CompiledBoard, compile};
use crate::map::{MapConstraints, MapDef, PlacedPiece};
use crate::terrain::{FeatureDef, TileDef};
use crate::vocab::{SourceLoc, Text};

/// One tile, `one`: a single grass cell one level thick.
struct Defs(TileDef);

impl BoardDefs for Defs {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        (name.as_str() == "one").then_some(&self.0)
    }
    fn feature(&self, _: &Sym) -> Option<&FeatureDef> {
        None
    }
}

/// A board with a one-level tile at each `(hex, level)`; each tile's top is a surface at
/// `level + 1` unless another tile sits directly on it.
fn board(tiles: &[(Hex, u8)]) -> CompiledBoard {
    let one = TileDef {
        id: DefId(0),
        name: Text::default(),
        terrain: DefId(1),
        thickness: 1,
        shape: Shape::new([Hex::ORIGIN]),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    let pieces = tiles.iter().map(|&(at, level)| PlacedPiece {
        piece: "one".into(),
        at,
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
    compile(&map, &Defs(one)).unwrap()
}

fn footprint(cells: impl IntoIterator<Item = Hex>, max_spread: u8) -> FootprintDef {
    FootprintDef {
        id: DefId(0),
        name: Text::default(),
        cells: Shape::new(cells),
        move_mode: MoveMode::Rigid,
        max_spread,
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    }
}

/// A board over every hex within three of the origin, most of them with one or two tiles, and an
/// anchor on one of the origin's surfaces, which always has one.
fn board_and_anchor() -> impl Strategy<Value = (CompiledBoard, Space)> {
    let levels = || prop::collection::btree_set(0u8..4, 1..3);
    let column = prop::option::weighted(0.9, levels());
    let others = prop::collection::vec(column, 37);
    (others, levels(), any::<prop::sample::Index>()).prop_map(|(others, origin, pick)| {
        let hexes = Hex::ORIGIN
            .spiral(3)
            .into_iter()
            .filter(|h| *h != Hex::ORIGIN);
        let columns = hexes
            .zip(others)
            .filter_map(|(hex, levels)| Some((hex, levels?)));
        let columns = columns.chain([(Hex::ORIGIN, origin)]);
        let tiles: Vec<(Hex, u8)> = columns
            .flat_map(|(hex, levels)| levels.into_iter().map(move |level| (hex, level)))
            .collect();
        let board = board(&tiles);
        let surfaces = board.surfaces(Hex::ORIGIN);
        let anchor = Space::at(Hex::ORIGIN, pick.get(surfaces).z);
        (board, anchor)
    })
}

/// A footprint of the origin and up to three other cells within two of it.
fn any_footprint() -> impl Strategy<Value = FootprintDef> {
    let near: Vec<Hex> = Hex::ORIGIN
        .spiral(2)
        .into_iter()
        .filter(|h| *h != Hex::ORIGIN)
        .collect();
    let offsets = prop::sample::subsequence(near, 0..4);
    (offsets, 0u8..4).prop_map(|(offsets, max_spread)| {
        footprint(offsets.into_iter().chain([Hex::ORIGIN]), max_spread)
    })
}

fn any_rotation() -> impl Strategy<Value = Rotation> {
    (0i32..6).prop_map(Rotation::from_steps)
}

proptest! {
    #[test]
    fn a_one_cell_footprint_covers_exactly_its_anchor(
        (board, anchor) in board_and_anchor(),
        rotation in any_rotation(),
    ) {
        let one = footprint([Hex::ORIGIN], 0);
        prop_assert_eq!(one.spaces(&board, anchor, Rotation::default()), Ok(vec![anchor]));
        prop_assert_eq!(one.spaces(&board, anchor, rotation), Ok(vec![anchor]));
    }

    #[test]
    fn there_is_one_space_per_cell(
        (board, anchor) in board_and_anchor(),
        fp in any_footprint(),
        rotation in any_rotation(),
    ) {
        if let Ok(spaces) = fp.spaces(&board, anchor, rotation) {
            prop_assert_eq!(spaces.len(), fp.cells.len());
        }
    }

    #[test]
    fn the_spread_never_exceeds_the_limit(
        (board, anchor) in board_and_anchor(),
        fp in any_footprint(),
        rotation in any_rotation(),
    ) {
        match fp.spaces(&board, anchor, rotation) {
            Ok(spaces) => {
                let levels: BTreeSet<u32> = spaces.iter().map(|s| s.z).collect();
                let spread = levels.last().unwrap() - levels.first().unwrap();
                prop_assert!(spread <= u32::from(fp.max_spread));
            }
            Err(FootprintError::Spread { spread, max }) => {
                prop_assert_eq!(max, fp.max_spread);
                prop_assert!(spread > u32::from(max));
            }
            Err(FootprintError::NoSurface(hex)) => prop_assert!(board.surfaces(hex).is_empty()),
            Err(FootprintError::NotASurface(_)) => prop_assert!(false, "the anchor is a surface"),
        }
    }

    #[test]
    fn the_spaces_lie_on_the_rotated_shape(
        (board, anchor) in board_and_anchor(),
        fp in any_footprint(),
        rotation in any_rotation(),
    ) {
        if let Ok(spaces) = fp.spaces(&board, anchor, rotation) {
            let hexes: Vec<Hex> = spaces.iter().map(|s| s.hex()).collect();
            let shape = fp.cells.rotated(rotation).translated(anchor.hex());
            prop_assert_eq!(hexes, shape.cells().to_vec());
            let standing = spaces.iter().all(|s| board.surfaces(s.hex()).iter().any(|f| f.z == s.z));
            prop_assert!(standing);
        }
    }
}

/// The anchor at (0, 0) stands at level 2; (1, 0) has surfaces at levels 1 and 3.
fn tied() -> CompiledBoard {
    let east = Hex::new(1, 0);
    board(&[(Hex::ORIGIN, 1), (east, 0), (east, 2)])
}

#[test]
fn a_cell_between_two_equally_near_surfaces_takes_the_lower() {
    let two = footprint([Hex::ORIGIN, Hex::new(1, 0)], 1);
    let spaces = two.spaces(&tied(), Space::new(0, 0, 2), Rotation::default());
    assert_eq!(spaces, Ok(vec![Space::new(0, 0, 2), Space::new(1, 0, 1)]));
}

#[test]
fn a_cell_with_no_surface_is_refused() {
    let two = footprint([Hex::ORIGIN, Hex::new(0, 1)], 1);
    let spaces = two.spaces(&tied(), Space::new(0, 0, 2), Rotation::default());
    assert_eq!(spaces, Err(FootprintError::NoSurface(Hex::new(0, 1))));
}

#[test]
fn an_anchor_off_the_surfaces_is_refused() {
    let one = footprint([Hex::ORIGIN], 0);
    let anchor = Space::new(0, 0, 1);
    let spaces = one.spaces(&tied(), anchor, Rotation::default());
    assert_eq!(spaces, Err(FootprintError::NotASurface(anchor)));
}

#[test]
fn rotation_moves_the_cells_with_the_shape() {
    let two = footprint([Hex::ORIGIN, Hex::new(1, 0)], 1);
    let turned = Rotation::from_steps(3);
    let spaces = two.spaces(&tied(), Space::new(0, 0, 2), turned);
    assert_eq!(spaces, Err(FootprintError::NoSurface(Hex::new(-1, 0))));
    let flat = board(&[(Hex::ORIGIN, 0), (Hex::new(-1, 0), 0)]);
    let spaces = two.spaces(&flat, Space::new(0, 0, 1), turned);
    assert_eq!(spaces, Ok(vec![Space::new(-1, 0, 1), Space::new(0, 0, 1)]));
}

#[test]
fn the_board_helper_puts_surfaces_on_tile_tops() {
    let board = tied();
    let levels = |hex: Hex| -> Vec<u32> { board.surfaces(hex).iter().map(|s| s.z).collect() };
    assert_eq!(levels(Hex::ORIGIN), vec![2]);
    assert_eq!(levels(Hex::new(1, 0)), vec![1, 3]);
}
