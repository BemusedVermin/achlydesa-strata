//! The placement tool: ghost level selection, placement feedback, brushes and undo.

use std::path::PathBuf;
use std::sync::Arc;

use strata_content::{ContentRegistry, load};
use strata_editor::{
    Brush, Edit, EditError, LevelMode, MapDocument, Mark, Metadata, OpenedMap, PaletteItem,
    Problem, RegistryDefs, Tool, ValidationOptions,
};
use strata_kernel::{DefId, Hex, Rotation, Space, Sym};
use strata_model::{MapConstraints, MapDef, PlacedPiece, SourceLoc, Text};

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/maps"))
}

fn registry() -> ContentRegistry {
    load(&[root()]).unwrap()
}

/// Opens the fixture map `t/maps/<name>.lua`.
fn open(name: &str) -> MapDocument<RegistryDefs> {
    let file = root().join(format!("t/maps/{name}.lua"));
    OpenedMap::open(&file).unwrap().into_document()
}

/// A document with no pieces, against the fixture pack.
fn empty() -> MapDocument<RegistryDefs> {
    let map = MapDef {
        id: DefId(0),
        name: Text("empty".into()),
        authors: Vec::new(),
        orientation: "pointy".into(),
        pieces: Vec::new(),
        features: Vec::new(),
        zones: Default::default(),
        relic_spots: Default::default(),
        constraints: MapConstraints::default(),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    MapDocument::new(map, RegistryDefs::new(registry(), "t".to_owned()))
}

fn tile(name: &str) -> PaletteItem {
    PaletteItem::Tile(name.into())
}

fn feature(name: &str) -> PaletteItem {
    PaletteItem::Feature(name.into())
}

fn tool(item: PaletteItem, brush: Brush, level: LevelMode) -> Tool {
    let mut tool = Tool::default();
    tool.item = Some(item);
    tool.brush = brush;
    tool.level = level;
    tool
}

fn mark(q: i32, r: i32, z: u32, ok: bool) -> Mark {
    Mark {
        space: Space::at(Hex::new(q, r), z),
        ok,
    }
}

fn hexes(cells: &[(i32, i32)]) -> Vec<Hex> {
    cells.iter().map(|&(q, r)| Hex::new(q, r)).collect()
}

fn placed(edit: &Edit) -> Vec<(Hex, u8)> {
    match edit {
        Edit::PlacePiece(p) => vec![(p.at, p.level)],
        Edit::Batch(edits) => edits.iter().flat_map(placed).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn the_ghost_snaps_onto_the_top_surface_under_the_cursor() {
    let mut doc = open("fair");
    // Let the pair overhang lower ground.
    let loose = MapConstraints::default();
    doc.apply(Edit::SetMetadata(Metadata::Constraints(loose)))
        .unwrap();
    // Anchored on the hill at (2, 1), top 2, the pair rests there and overhangs the grass at
    // (3, 1), top 1.
    let preview =
        tool(tile("grass_2"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::new(2, 1));
    let Some(Edit::PlacePiece(piece)) = preview.edit() else {
        panic!("{preview:?}");
    };
    assert_eq!(piece.level, 2);
    let tops: Vec<Space> = preview.marks.iter().map(|m| m.space).collect();
    assert_eq!(tops, vec![Space::new(2, 1, 3), Space::new(3, 1, 3)]);
    // Anchored on the grass at (1, 1), top 1, it rests at 1 and runs into the hill: only the
    // cursor's column decides the level.
    let preview =
        tool(tile("grass_2"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::new(1, 1));
    assert_eq!(preview.problems, vec![Problem::Overlap(hexes(&[(2, 1)]))]);
    assert_eq!(
        preview.marks,
        vec![mark(1, 1, 2, false), mark(2, 1, 2, false)]
    );
    assert_eq!(preview.edit(), None);
}

#[test]
fn under_a_bridge_the_ghost_snaps_onto_the_deck_and_a_locked_level_builds_beneath() {
    let mut doc = empty();
    let grass = |level| {
        Edit::PlacePiece(PlacedPiece {
            piece: "grass_1".into(),
            at: Hex::ORIGIN,
            rot: Rotation::default(),
            level,
        })
    };
    // Ground at level 0 and a deck at level 3, with open air between.
    doc.apply(Edit::Batch(vec![grass(0), grass(3)])).unwrap();
    let snapped = tool(tile("grass_1"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::ORIGIN);
    assert_eq!(placed(&snapped.edit().unwrap()), vec![(Hex::ORIGIN, 4)]);
    let locked =
        tool(tile("grass_1"), Brush::Single, LevelMode::Locked(1)).preview(&doc, Hex::ORIGIN);
    assert_eq!(locked.problems, vec![]);
    assert_eq!(placed(&locked.edit().unwrap()), vec![(Hex::ORIGIN, 1)]);
}

#[test]
fn with_support_required_a_multi_hex_tile_over_uneven_ground_is_refused_where_it_floats() {
    let doc = open("fair");
    assert!(doc.map().constraints.require_support);
    // On the hill at (2, 1) the pair's second hex, (3, 1), is a level above its grass.
    let preview =
        tool(tile("grass_2"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::new(2, 1));
    assert_eq!(
        preview.problems,
        vec![Problem::Unsupported(hexes(&[(3, 1)]))]
    );
    assert_eq!(preview.edit(), None);
    // On level ground it fits.
    let preview =
        tool(tile("grass_2"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::new(3, 1));
    assert_eq!(preview.problems, vec![]);
    assert_eq!(placed(&preview.edit().unwrap()), vec![(Hex::new(3, 1), 1)]);
}

#[test]
fn the_ghost_on_open_ground_rests_on_level_zero() {
    let doc = open("fair");
    let preview =
        tool(tile("hill_2"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::new(9, 9));
    assert_eq!(preview.marks, vec![mark(9, 9, 2, true)]);
    assert_eq!(placed(&preview.edit().unwrap()), vec![(Hex::new(9, 9), 0)]);
}

#[test]
fn a_locked_level_ignores_the_terrain_under_the_ghost() {
    // Without `require_support`, a locked level may float above the grass at (0, 0).
    let doc = open("apart");
    assert!(!doc.map().constraints.require_support);
    let snapped = tool(tile("grass_1"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::ORIGIN);
    assert_eq!(placed(&snapped.edit().unwrap()), vec![(Hex::ORIGIN, 1)]);
    let locked =
        tool(tile("grass_1"), Brush::Single, LevelMode::Locked(4)).preview(&doc, Hex::ORIGIN);
    assert_eq!(locked.problems, vec![]);
    assert_eq!(locked.marks, vec![mark(0, 0, 5, true)]);
    assert_eq!(placed(&locked.edit().unwrap()), vec![(Hex::ORIGIN, 4)]);
}

#[test]
fn a_ghost_that_overlaps_or_floats_is_refused_with_the_hexes_at_fault() {
    let doc = open("fair");
    let preview =
        tool(tile("grass_2"), Brush::Single, LevelMode::Locked(1)).preview(&doc, Hex::new(1, 1));
    // (2, 1) is a hill spanning levels 0 to 2; (1, 1) has its top at 1, so only the hill clashes,
    // and nothing in the hill's column ends at level 1 to hold the piece up.
    assert_eq!(
        preview.problems,
        vec![
            Problem::Overlap(hexes(&[(2, 1)])),
            Problem::Unsupported(hexes(&[(2, 1)]))
        ]
    );
    assert_eq!(preview.edit(), None);
    assert!(preview.marks.iter().all(|m| !m.ok));

    let preview =
        tool(tile("grass_1"), Brush::Single, LevelMode::Locked(4)).preview(&doc, Hex::new(0, 0));
    assert_eq!(
        preview.problems,
        vec![Problem::Unsupported(hexes(&[(0, 0)]))]
    );
    assert_eq!(preview.edit(), None);
}

#[test]
fn rotation_turns_the_ghost_about_its_anchor() {
    let doc = empty();
    let mut turned = tool(tile("grass_2"), Brush::Single, LevelMode::Snap);
    turned.rotate(-1);
    assert_eq!(turned.rot, Rotation::from_steps(5));
    turned.rotate(2);
    let preview = turned.preview(&doc, Hex::new(3, 3));
    let second = Hex::new(1, 0)
        .rotate(Rotation::from_steps(1))
        .add(Hex::new(3, 3));
    let cells: Vec<Hex> = preview.marks.iter().map(|m| m.space.hex()).collect();
    assert_eq!(cells, vec![Hex::new(3, 3), second]);
    let Some(Edit::PlacePiece(piece)) = preview.edit() else {
        panic!("{preview:?}");
    };
    assert_eq!(piece.rot, Rotation::from_steps(1));
}

#[test]
fn snapping_above_the_highest_level_is_refused() {
    let mut doc = empty();
    let top = Edit::PlacePiece(PlacedPiece {
        piece: "hill_2".into(),
        at: Hex::ORIGIN,
        rot: Rotation::default(),
        level: 255,
    });
    doc.apply(top).unwrap();
    let preview = tool(tile("grass_1"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::ORIGIN);
    assert_eq!(preview.problems, vec![Problem::TooHigh(vec![Hex::ORIGIN])]);
    assert_eq!(preview.marks, vec![mark(0, 0, 257, false)]);
    assert_eq!(preview.edit(), None);
}

#[test]
fn a_feature_stands_on_terrain_and_is_refused_off_the_board() {
    let doc = open("fair");
    let on = tool(feature("tree"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::new(2, 1));
    // The tree is two levels tall on the hill's top at 2.
    assert_eq!(on.marks, vec![mark(2, 1, 4, true)]);
    assert!(matches!(on.edit(), Some(Edit::PlaceFeature(_))));

    let off = tool(feature("wall"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::new(4, 0));
    assert_eq!(off.problems, vec![Problem::OffBoard(hexes(&[(5, 0)]))]);
    assert_eq!(off.edit(), None);
}

#[test]
fn running_out_of_stock_warns_but_still_places() {
    // The box holds three grass tiles and the map already uses four.
    let doc = open("inventory");
    let preview =
        tool(tile("grass_1"), Brush::Single, LevelMode::Snap).preview(&doc, Hex::new(0, 5));
    assert_eq!(
        preview.problems,
        vec![Problem::OutOfStock { remaining: -2 }]
    );
    assert!(preview.edit().is_some());
    // A full stroke reports what is left after all of it.
    let line = tool(tile("water_1"), Brush::Line, LevelMode::Snap);
    let mut line = line;
    line.press(Hex::new(0, 5));
    let preview = line.preview(&doc, Hex::new(2, 5));
    assert_eq!(
        preview.problems,
        vec![Problem::OutOfStock { remaining: -3 }]
    );
    assert_eq!(preview.edits.len(), 3);
}

#[test]
fn a_line_stroke_is_one_edit_and_one_undo() {
    let mut doc = empty();
    let mut line = tool(tile("grass_1"), Brush::Line, LevelMode::Snap);
    line.press(Hex::new(0, 0));
    assert_eq!(line.drag_start(), Some(Hex::new(0, 0)));
    let preview = line.release(&doc, Hex::new(3, 0));
    assert_eq!(line.drag_start(), None);
    let edit = preview.edit().unwrap();
    let cells = placed(&edit);
    assert_eq!(
        cells,
        vec![
            (Hex::new(0, 0), 0),
            (Hex::new(1, 0), 0),
            (Hex::new(2, 0), 0),
            (Hex::new(3, 0), 0)
        ]
    );
    doc.apply(edit).unwrap();
    assert_eq!(doc.map().pieces.len(), 4);
    assert!(doc.board().is_ok());
    doc.undo().unwrap();
    assert!(doc.map().pieces.is_empty());
    assert!(!doc.can_undo());
    doc.redo().unwrap();
    assert_eq!(doc.map().pieces.len(), 4);
}

#[test]
fn each_hex_of_a_stroke_snaps_on_its_own_and_blocked_hexes_are_skipped() {
    let doc = open("fair");
    let mut line = tool(tile("grass_1"), Brush::Line, LevelMode::Snap);
    line.press(Hex::new(0, 1));
    let preview = line.preview(&doc, Hex::new(4, 1));
    let levels: Vec<u8> = placed(&preview.edit().unwrap())
        .iter()
        .map(|p| p.1)
        .collect();
    assert_eq!(levels, vec![1, 1, 2, 1, 1]);

    let mut low = tool(tile("grass_1"), Brush::Line, LevelMode::Locked(0));
    low.press(Hex::new(3, 1));
    let preview = low.preview(&doc, Hex::new(6, 1));
    assert_eq!(
        preview.problems,
        vec![Problem::Overlap(hexes(&[(3, 1), (4, 1)]))]
    );
    let cells: Vec<Hex> = placed(&preview.edit().unwrap())
        .iter()
        .map(|p| p.0)
        .collect();
    assert_eq!(cells, hexes(&[(5, 1), (6, 1)]));
    let ok: Vec<bool> = preview.marks.iter().map(|m| m.ok).collect();
    assert_eq!(ok, vec![false, false, true, true]);
}

#[test]
fn a_fill_covers_the_connected_hexes_with_the_same_top() {
    let mut doc = open("fair");
    let fill = tool(tile("grass_1"), Brush::Fill, LevelMode::Snap);
    // Water at (2, 0) and the hill at (2, 1) cut the west grass off from the east grass.
    let preview = fill.preview(&doc, Hex::new(0, 0));
    let edit = preview.edit().unwrap();
    let expected: Vec<(Hex, u8)> = hexes(&[(0, 0), (1, 0), (0, 1), (1, 1)])
        .into_iter()
        .map(|h| (h, 1))
        .collect();
    let mut cells = placed(&edit);
    cells.sort();
    let mut expected = expected;
    expected.sort();
    assert_eq!(cells, expected);
    doc.apply(edit).unwrap();
    assert!(doc.board().is_ok());
}

#[test]
fn a_fill_on_open_ground_fills_holes_only() {
    let mut doc = open("fair");
    // Off the map, the fill would have no bounds.
    let fill = tool(tile("grass_1"), Brush::Fill, LevelMode::Snap);
    let preview = fill.preview(&doc, Hex::new(9, 9));
    assert_eq!(preview.problems, vec![Problem::NotEnclosed]);
    assert_eq!(preview.marks, vec![mark(9, 9, 0, false)]);
    assert_eq!(preview.edit(), None);
    // Wall in (2, 2) with grass on every side, and the fill finds it.
    // Its neighbors at r = 1 are already terrain; add the four below.
    let around: Vec<Edit> = Hex::new(2, 2)
        .neighbors()
        .into_iter()
        .filter(|h| h.r > 1)
        .map(|at| {
            Edit::PlacePiece(PlacedPiece {
                piece: "grass_1".into(),
                at,
                rot: Rotation::default(),
                level: 0,
            })
        })
        .collect();
    doc.apply(Edit::Batch(around)).unwrap();
    let preview = fill.preview(&doc, Hex::new(2, 2));
    assert_eq!(preview.problems, vec![]);
    assert_eq!(placed(&preview.edit().unwrap()), vec![(Hex::new(2, 2), 0)]);
}

#[test]
fn line_and_fill_paint_one_hex_tiles_only() {
    let doc = empty();
    for item in [tile("grass_2"), feature("tree")] {
        let preview = tool(item, Brush::Fill, LevelMode::Snap).preview(&doc, Hex::ORIGIN);
        assert_eq!(preview.problems, vec![Problem::NotOneHex]);
        assert_eq!(preview.edit(), None);
    }
    let preview = tool(tile("nothing"), Brush::Line, LevelMode::Snap).preview(&doc, Hex::ORIGIN);
    assert_eq!(preview.problems, vec![Problem::UnknownItem]);
}

#[test]
fn with_nothing_selected_the_tool_places_nothing() {
    let doc = open("fair");
    let preview = Tool::default().preview(&doc, Hex::ORIGIN);
    assert_eq!(preview.marks, vec![]);
    assert_eq!(preview.edit(), None);
}

#[test]
fn a_batch_that_fails_part_way_leaves_the_document_unchanged() {
    let mut doc = open("fair");
    let before = doc.map().clone();
    let board = doc.board().unwrap().clone();
    let validation = doc.validate(&ValidationOptions::default());
    // Zone, relic, metadata and piece edits, some in a nested batch, all applied before the
    // last edit fails.
    let batch = Edit::Batch(vec![
        Edit::RemovePiece { index: 0 },
        Edit::PaintZone {
            zone: "start_c".into(),
            cells: hexes(&[(1, 0)]),
        },
        Edit::Batch(vec![
            Edit::SetMetadata(Metadata::Name(Text("Renamed".into()))),
            Edit::SetMetadata(Metadata::Constraints(MapConstraints::default())),
            Edit::SetRelicSpot {
                name: "hill".into(),
                at: None,
            },
            Edit::Batch(vec![Edit::RemovePiece { index: 0 }]),
        ]),
        Edit::RemovePiece { index: 99 },
    ]);
    assert_eq!(doc.apply(batch), Err(EditError::NoSuchPiece(99)));
    assert_eq!(doc.map(), &before);
    assert_eq!(doc.board(), Ok(&board));
    // Nothing changed, so the cached validation still stands.
    let again = doc.validate(&ValidationOptions::default());
    assert!(Arc::ptr_eq(&validation, &again));
    assert!(!doc.can_undo());
    // A failure inside a nested batch rolls back the outer batch too.
    let nested = Edit::Batch(vec![
        Edit::SetMetadata(Metadata::Name(Text("Renamed".into()))),
        Edit::Batch(vec![
            Edit::RemovePiece { index: 0 },
            Edit::RemoveFeature { index: 0 },
        ]),
    ]);
    assert_eq!(doc.apply(nested), Err(EditError::NoSuchFeature(0)));
    assert_eq!(doc.map(), &before);
    assert_eq!(doc.board(), Ok(&board));
}

#[test]
fn a_batch_with_zone_and_metadata_edits_undoes_and_redoes_as_one_step() {
    let mut doc = open("fair");
    let before = doc.map().clone();
    let batch = Edit::Batch(vec![
        Edit::PaintZone {
            zone: "start_c".into(),
            cells: hexes(&[(1, 0)]),
        },
        Edit::Batch(vec![
            Edit::SetMetadata(Metadata::Name(Text("Renamed".into()))),
            Edit::RemovePiece { index: 9 },
        ]),
    ]);
    doc.apply(batch).unwrap();
    let after = doc.map().clone();
    assert_eq!(after.name, Text("Renamed".into()));
    assert_eq!(after.pieces.len(), before.pieces.len() - 1);
    assert!(after.zones.contains_key(&Sym::from("start_c")));
    doc.undo().unwrap();
    assert_eq!(doc.map(), &before);
    assert!(!doc.can_undo());
    doc.redo().unwrap();
    assert_eq!(doc.map(), &after);
}

#[test]
fn an_empty_batch_is_refused_and_leaves_no_undo_step() {
    let mut doc = open("fair");
    let before = doc.map().clone();
    assert_eq!(doc.apply(Edit::Batch(vec![])), Err(EditError::EmptyBatch));
    let nested = Edit::Batch(vec![Edit::Batch(vec![]), Edit::Batch(vec![])]);
    assert_eq!(doc.apply(nested), Err(EditError::EmptyBatch));
    assert_eq!(doc.map(), &before);
    assert!(!doc.can_undo());
}
