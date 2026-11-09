//! Placing footprints on small boards: surfaces, level spread and bridges.

use std::collections::BTreeMap;

use strata_kernel::{DefId, Hex, Rotation, Shape, Space, Sym};

use super::{FootprintDef, MoveMode};
use crate::board::{BoardDefs, CompiledBoard, Surface, compile};
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

#[test]
fn a_cell_falls_back_to_the_next_surface_that_fits() {
    // The anchor stands at 2. Column (1, 0) has ground at 1, under an overhang with one free
    // level, and the overhang's top at 3: both one level from the anchor, the lower first.
    let board = board(&[(0, 0, 1), (1, 0, 0), (1, 0, 2)]);
    let at = Space::new(0, 0, 2);
    let rotation = Rotation::default();
    let roomy = |_: Space, s: &Surface| s.headroom.is_none_or(|free| free >= 2);
    let nearest = spaces(&two(1), &board, at, 0);
    assert_eq!(nearest, Some(vec![at, Space::new(1, 0, 1)]));
    let fitted = two(1).spaces_where(&board, at, rotation, roomy);
    let fitted = fitted.map(|found| found.into_iter().map(|(s, _)| s).collect::<Vec<_>>());
    assert_eq!(fitted, Some(vec![at, Space::new(1, 0, 3)]));
    // The fallback stays within the spread.
    let board = board_with_gap();
    let fitted = two(1).spaces_where(&board, Space::new(0, 0, 2), rotation, roomy);
    assert_eq!(fitted, None, "the only roomy surface is two levels up");
    assert!(
        two(2)
            .spaces_where(&board, Space::new(0, 0, 2), rotation, roomy)
            .is_some()
    );
}

/// Like the overhang board, but the overhang is two levels thick, so its top is at 4: two
/// levels above the anchor.
fn board_with_gap() -> CompiledBoard {
    board(&[(0, 0, 1), (1, 0, 0), (1, 0, 2), (1, 0, 3)])
}

#[test]
fn the_anchor_never_falls_back() {
    // The anchor's column has surfaces at 1 and 3; only 3 fits, but the anchor is asked at 1.
    let board = board(&[(0, 0, 0), (0, 0, 2), (1, 0, 0), (1, 0, 2)]);
    let high = |s: Space, _: &Surface| s.z == 3;
    let at = Space::new(0, 0, 1);
    assert_eq!(
        two(2).spaces_where(&board, at, Rotation::default(), high),
        None
    );
}

/// The placement as it was before cells could fall back: the nearest surface, then the spread.
fn nearest_only(def: &FootprintDef, board: &CompiledBoard, at: Space, o: u8) -> Option<Vec<Space>> {
    board.surfaces(at.hex()).iter().find(|s| s.z == at.z)?;
    let mut placed = Vec::new();
    for hex in def.cells_at(at.hex(), Rotation::new(o).unwrap()) {
        let surfaces = board.surfaces(hex).iter();
        let nearest = surfaces.min_by_key(|s| (s.z.abs_diff(at.z), s.z))?;
        placed.push(Space::at(hex, nearest.z));
    }
    let low = placed.iter().map(|s| s.z).min()?;
    let high = placed.iter().map(|s| s.z).max()?;
    (high - low <= u32::from(def.max_spread)).then_some(placed)
}

#[test]
fn when_every_surface_fits_the_placement_is_the_nearest_one() {
    // Many ragged seven-column boards, some columns with a second surface above the first.
    let hexes = Hex::ORIGIN.spiral(1);
    for variant in 0u32..300 {
        let mut pieces = Vec::new();
        for (i, h) in (0u32..).zip(&hexes) {
            let mix = variant.wrapping_mul(2_654_435_761).rotate_left(i * 5) ^ (i * 97);
            let level = (mix % 4) as u8;
            pieces.push((h.q, h.r, level));
            let gap = ((mix >> 3) % 4) as u8;
            if gap > 0 {
                pieces.push((h.q, h.r, level + gap));
            }
        }
        let board = board(&pieces);
        for spread in 0..3 {
            let def = two(spread);
            for surface in board.surfaces(Hex::ORIGIN) {
                let at = Space::at(Hex::ORIGIN, surface.z);
                for o in 0..6 {
                    let want = nearest_only(&def, &board, at, o);
                    assert_eq!(spaces(&def, &board, at, o), want, "{pieces:?} {at:?} {o}");
                }
            }
        }
    }
}
