use std::collections::BTreeMap;

use strata_kernel::{BlockerFootprint, DefId, Hex, Rotation, Shape, Sym, blocked};
use strata_kernel::{Point, Point3, SUB_LEVELS, Seg3};

use super::{BoardDefs, BoardError, CompiledBoard, Surface, compile};
use crate::map::{MapConstraints, MapDef, PlacedFeature, PlacedPiece};
use crate::terrain::{FeatureCell, FeatureDef, Segment, TileDef};
use crate::vocab::{SourceLoc, Text};

const GRASS: DefId = DefId(1);
const ROCK: DefId = DefId(2);

#[derive(Default)]
struct Defs {
    tiles: BTreeMap<Sym, TileDef>,
    features: BTreeMap<Sym, FeatureDef>,
}

impl BoardDefs for Defs {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        self.tiles.get(name)
    }
    fn feature(&self, name: &Sym) -> Option<&FeatureDef> {
        self.features.get(name)
    }
}

fn tile(terrain: DefId, thickness: u8, cells: &[(i32, i32)]) -> TileDef {
    TileDef {
        id: DefId(0),
        name: Text::default(),
        terrain,
        thickness,
        shape: Shape::new(cells.iter().map(|&(q, r)| Hex::new(q, r))),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    }
}

fn wall(blocks_los: bool) -> FeatureDef {
    let segment = Segment {
        from: 0,
        to: 2,
        top: true,
    };
    FeatureDef {
        id: DefId(0),
        name: Text::default(),
        blocks_los,
        cells: vec![
            FeatureCell {
                at: Hex::new(0, 0),
                segments: vec![segment.clone()],
            },
            FeatureCell {
                at: Hex::new(1, 0),
                segments: vec![segment],
            },
        ],
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    }
}

fn defs() -> Defs {
    let mut defs = Defs::default();
    let l_shape = [(0, 0), (1, 0), (0, 1)];
    defs.tiles.insert("one".into(), tile(GRASS, 1, &[(0, 0)]));
    defs.tiles.insert("slab".into(), tile(GRASS, 1, &l_shape));
    defs.tiles.insert("block".into(), tile(ROCK, 3, &l_shape));
    defs.tiles.insert("deck".into(), tile(ROCK, 1, &[(0, 0)]));
    defs.features.insert("wall".into(), wall(true));
    defs.features.insert("rail".into(), wall(false));
    defs
}

fn piece(name: &str, at: (i32, i32), rot: u8, level: u8) -> PlacedPiece {
    PlacedPiece {
        piece: name.into(),
        at: Hex::new(at.0, at.1),
        rot: Rotation::from_steps(rot.into()),
        level,
    }
}

fn map(pieces: Vec<PlacedPiece>, features: Vec<PlacedFeature>, require_support: bool) -> MapDef {
    MapDef {
        id: DefId(0),
        name: Text::default(),
        authors: Vec::new(),
        orientation: "pointy".into(),
        pieces,
        features,
        zones: Default::default(),
        relic_spots: Default::default(),
        constraints: MapConstraints {
            inventory: None,
            require_support,
        },
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    }
}

fn feature(name: &str, at: (i32, i32), rot: u8) -> PlacedFeature {
    PlacedFeature {
        feature: name.into(),
        at: Hex::new(at.0, at.1),
        rot: Rotation::from_steps(rot.into()),
    }
}

#[test]
fn bridge_yields_two_surfaces() {
    let pieces = vec![piece("one", (0, 0), 0, 0), piece("deck", (0, 0), 0, 4)];
    let board = compile(&map(pieces, vec![], false), &defs()).unwrap();
    let hex = Hex::new(0, 0);
    let surfaces = board.surfaces(hex);
    assert_eq!(
        surfaces,
        [
            Surface {
                z: 1,
                headroom: Some(3),
                terrain: GRASS
            },
            Surface {
                z: 5,
                headroom: None,
                terrain: ROCK
            },
        ]
    );
    assert_eq!(board.top_surface(hex), Some(surfaces[1]));
    assert!(board.is_solid(hex, 0) && !board.is_solid(hex, 1) && board.is_solid(hex, 4));
}

#[test]
fn stacked_segments_have_no_surface_between() {
    let pieces = vec![piece("one", (0, 0), 0, 0), piece("deck", (0, 0), 0, 1)];
    let board = compile(&map(pieces, vec![], false), &defs()).unwrap();
    assert_eq!(board.surfaces(Hex::new(0, 0)).len(), 1);
    assert_eq!(board.top_surface(Hex::new(0, 0)).unwrap().z, 2);
}

#[test]
fn bounding_box_columns_without_pieces_are_empty() {
    let pieces = vec![piece("one", (0, 0), 0, 0), piece("one", (3, 2), 0, 0)];
    let board = compile(&map(pieces, vec![], false), &defs()).unwrap();
    assert_eq!(board.column(Hex::new(1, 1)).unwrap().segments, []);
    assert!(board.column(Hex::new(4, 0)).is_none());
    assert!(board.surfaces(Hex::new(9, 9)).is_empty());
    assert_eq!(board.top_surface(Hex::new(1, 1)), None);
}

#[test]
fn overlap_reports_piece_and_hex() {
    let pieces = vec![piece("one", (0, 0), 0, 0), piece("slab", (-1, 0), 0, 0)];
    let err = compile(&map(pieces, vec![], false), &defs()).unwrap_err();
    assert_eq!(
        err,
        BoardError::Overlap {
            piece: 1,
            other: 0,
            hex: Hex::new(0, 0)
        }
    );
}

#[test]
fn unsupported_piece_reports_piece_and_hex() {
    let pieces = vec![piece("one", (0, 0), 0, 0), piece("deck", (0, 0), 0, 4)];
    let err = compile(&map(pieces.clone(), vec![], true), &defs()).unwrap_err();
    assert_eq!(
        err,
        BoardError::Unsupported {
            piece: 1,
            hex: Hex::new(0, 0)
        }
    );
    let supported = vec![piece("one", (0, 0), 0, 0), piece("deck", (0, 0), 0, 1)];
    assert!(compile(&map(supported, vec![], true), &defs()).is_ok());
}

#[test]
fn unknown_names_are_errors() {
    let err = compile(
        &map(vec![piece("nope", (2, 1), 0, 0)], vec![], false),
        &defs(),
    );
    assert!(matches!(err, Err(BoardError::UnknownTile { piece: 0, .. })));
    let m = map(
        vec![piece("one", (0, 0), 0, 0)],
        vec![feature("nope", (0, 0), 0)],
        false,
    );
    assert!(matches!(
        compile(&m, &defs()),
        Err(BoardError::UnknownFeature { feature: 0, .. })
    ));
}

#[test]
fn feature_off_the_board_is_an_error() {
    let m = map(
        vec![piece("one", (0, 0), 0, 0)],
        vec![feature("wall", (0, 0), 0)],
        false,
    );
    let err = compile(&m, &defs()).unwrap_err();
    assert_eq!(
        err,
        BoardError::FeatureOffBoard {
            feature: 0,
            hex: Hex::new(1, 0)
        }
    );
}

#[test]
fn features_stand_on_the_terrain_and_block_sight() {
    let pieces = vec![piece("slab", (0, 0), 0, 1), piece("one", (0, 0), 0, 0)];
    let m = map(pieces, vec![feature("wall", (0, 0), 0)], false);
    let board = compile(&m, &defs()).unwrap();
    let column = board.column(Hex::new(0, 0)).unwrap();
    assert_eq!(column.features.len(), 1);
    assert_eq!((column.features[0].z0, column.features[0].z1), (2, 4));
    assert!(column.features[0].top && column.features[0].blocks_los);
    assert!(board.is_solid(Hex::new(0, 0), 3));
    let top = board.blockers(Hex::new(0, 0)).last().unwrap();
    assert_eq!((top.z0, top.z1), (2 * SUB_LEVELS, 4 * SUB_LEVELS));
    assert_eq!(top.footprint, BlockerFootprint::FullHex);
}

#[test]
fn blockers_feed_the_kernel_occlusion_test() {
    let pieces = vec![piece("slab", (0, 0), 0, 0), piece("block", (2, 0), 0, 0)];
    let m = map(pieces, vec![], false);
    let board = compile(&m, &defs()).unwrap();
    let lattice = |hex: Hex| Point::center(hex);
    let at = |hex: Hex, z: i64| Point3::new(lattice(hex), z);
    let (a, b) = (Hex::new(0, 0), Hex::new(0, 1));
    let low = Seg3::new(at(a, 2 * SUB_LEVELS), at(b, 2 * SUB_LEVELS));
    let get = |h: Hex| board.blockers(h);
    assert!(!blocked(low, get, &[a, b]));
    let across = Seg3::new(at(a, SUB_LEVELS), at(Hex::new(3, 0), SUB_LEVELS));
    assert!(blocked(across, get, &[a, Hex::new(3, 0)]));
}

#[test]
fn compiling_twice_gives_equal_boards() {
    let pieces = vec![piece("slab", (0, 0), 2, 0), piece("block", (4, -1), 1, 1)];
    let m = map(pieces, vec![], false);
    assert_eq!(compile(&m, &defs()), compile(&m, &defs()));
}

#[test]
fn rotating_every_piece_rotates_the_board() {
    let pieces = vec![
        piece("slab", (0, 0), 0, 0),
        piece("block", (3, -1), 2, 0),
        piece("deck", (0, 0), 4, 1),
    ];
    let features = vec![feature("wall", (3, -1), 2)];
    let original = compile(&map(pieces.clone(), features.clone(), true), &defs()).unwrap();
    for steps in 1..6u8 {
        let by = Rotation::from_steps(steps.into());
        let turned_pieces = pieces
            .iter()
            .map(|p| PlacedPiece {
                at: p.at.rotate(by),
                rot: Rotation::from_steps(i32::from(p.rot.steps()) + i32::from(steps)),
                ..p.clone()
            })
            .collect();
        let turned_features = features
            .iter()
            .map(|f| PlacedFeature {
                at: f.at.rotate(by),
                rot: Rotation::from_steps(i32::from(f.rot.steps()) + i32::from(steps)),
                ..f.clone()
            })
            .collect();
        let turned = compile(&map(turned_pieces, turned_features, true), &defs()).unwrap();
        assert_same_columns(&original, &turned, by);
    }
}

/// Checks every column of `a`, moved by `by`, equals the column of `b`.
fn assert_same_columns(a: &CompiledBoard, b: &CompiledBoard, by: Rotation) {
    let mut seen = 0;
    for q in -8..=8 {
        for r in -8..=8 {
            let hex = Hex::new(q, r);
            let (left, right) = (a.column(hex), b.column(hex.rotate(by)));
            let empty = |c: Option<&super::Column>| c.is_none_or(|c| c.segments.is_empty());
            assert_eq!(empty(left), empty(right));
            if !empty(left) {
                assert_eq!(left, right, "column {hex:?} rotated by {by:?}");
                seen += 1;
            }
        }
    }
    assert!(seen > 0);
}
