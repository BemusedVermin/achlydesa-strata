use std::collections::BTreeMap;

use proptest::prelude::*;
use strata_editor::{Edit, EditError, MapDocument, Metadata};
use strata_kernel::{DefId, Hex, Rotation, Shape, Sym, Value};
use strata_model::board::BoardDefs;
use strata_model::{
    FeatureCell, FeatureDef, MapConstraints, MapDef, PlacedFeature, PlacedPiece, Segment,
    SourceLoc, Text, TileDef,
};

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

fn defs() -> Defs {
    let mut defs = Defs::default();
    defs.tiles.insert(
        "grass".into(),
        TileDef {
            id: DefId(0),
            name: Text::default(),
            terrain: DefId(1),
            thickness: 1,
            shape: Shape::new([Hex::new(0, 0), Hex::new(1, 0)]),
            tags: Default::default(),
            meta: Default::default(),
            source: SourceLoc::default(),
        },
    );
    defs.features.insert(
        "rock".into(),
        FeatureDef {
            id: DefId(0),
            name: Text::default(),
            blocks_los: true,
            cells: vec![FeatureCell {
                at: Hex::new(0, 0),
                segments: vec![Segment {
                    from: 0,
                    to: 1,
                    top: true,
                }],
            }],
            tags: Default::default(),
            meta: Default::default(),
            source: SourceLoc::default(),
        },
    );
    defs
}

fn empty_map() -> MapDef {
    MapDef {
        id: DefId(0),
        name: Text("test".into()),
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
    }
}

fn piece(q: i32, r: i32, rot: i32, level: u8) -> PlacedPiece {
    PlacedPiece {
        piece: "grass".into(),
        at: Hex::new(q, r),
        rot: Rotation::from_steps(rot),
        level,
    }
}

/// A random edit request; indices are reduced modulo the current length when it is resolved.
#[derive(Clone, Debug)]
enum Step {
    Place(i32, i32, i32, u8),
    Remove(usize),
    Move(usize, i32, i32, u8),
    Rotate(usize, i32),
    PlaceFeature(i32, i32),
    RemoveFeature(usize),
    Paint(u8, Vec<(i32, i32)>),
    Erase(u8, Vec<(i32, i32)>),
    Relic(u8, Option<(i32, i32)>),
    Name(String),
    Entry(u8, Option<i64>),
    /// Several steps as one [`Edit::Batch`], each resolved against the map the ones before it
    /// leave.
    Batch(Vec<Step>),
}

/// Any step, including batches nested up to three deep.
fn step() -> impl Strategy<Value = Step> {
    single().prop_recursive(3, 24, 6, |inner| {
        prop::collection::vec(inner, 0..6).prop_map(Step::Batch)
    })
}

/// A step that is not a batch.
fn single() -> impl Strategy<Value = Step> {
    let c = -4..4i32;
    let cells = prop::collection::vec((c.clone(), c.clone()), 0..5);
    prop_oneof![
        (c.clone(), c.clone(), 0..6i32, 0..3u8).prop_map(|(q, r, t, l)| Step::Place(q, r, t, l)),
        any::<usize>().prop_map(Step::Remove),
        (any::<usize>(), c.clone(), c.clone(), 0..3u8)
            .prop_map(|(i, q, r, l)| Step::Move(i, q, r, l)),
        (any::<usize>(), 0..6i32).prop_map(|(i, t)| Step::Rotate(i, t)),
        (c.clone(), c.clone()).prop_map(|(q, r)| Step::PlaceFeature(q, r)),
        any::<usize>().prop_map(Step::RemoveFeature),
        (0..3u8, cells.clone()).prop_map(|(z, v)| Step::Paint(z, v)),
        (0..3u8, cells).prop_map(|(z, v)| Step::Erase(z, v)),
        (0..3u8, prop::option::of((c.clone(), c))).prop_map(|(n, h)| Step::Relic(n, h)),
        "[a-z]{0,6}".prop_map(Step::Name),
        (0..3u8, prop::option::of(any::<i64>())).prop_map(|(k, v)| Step::Entry(k, v)),
    ]
}

fn hexes(cells: &[(i32, i32)]) -> Vec<Hex> {
    cells.iter().map(|&(q, r)| Hex::new(q, r)).collect()
}

fn resolve(step: &Step, map: &MapDef) -> Option<Edit> {
    let pick = |i: usize, len: usize| (len > 0).then(|| i % len);
    Some(match step {
        Step::Place(q, r, t, l) => Edit::PlacePiece(piece(*q, *r, *t, *l)),
        Step::Remove(i) => Edit::RemovePiece {
            index: pick(*i, map.pieces.len())?,
        },
        Step::Move(i, q, r, l) => Edit::MovePiece {
            index: pick(*i, map.pieces.len())?,
            at: Hex::new(*q, *r),
            level: *l,
        },
        Step::Rotate(i, t) => Edit::RotatePiece {
            index: pick(*i, map.pieces.len())?,
            rot: Rotation::from_steps(*t),
        },
        Step::PlaceFeature(q, r) => Edit::PlaceFeature(PlacedFeature {
            feature: "rock".into(),
            at: Hex::new(*q, *r),
            rot: Rotation::default(),
        }),
        Step::RemoveFeature(i) => Edit::RemoveFeature {
            index: pick(*i, map.features.len())?,
        },
        Step::Paint(z, v) => Edit::PaintZone {
            zone: format!("z{z}").into(),
            cells: hexes(v),
        },
        Step::Erase(z, v) => Edit::EraseZone {
            zone: format!("z{z}").into(),
            cells: hexes(v),
        },
        Step::Relic(n, h) => Edit::SetRelicSpot {
            name: format!("s{n}").into(),
            at: h.map(|(q, r)| Hex::new(q, r)),
        },
        Step::Name(s) => Edit::SetMetadata(Metadata::Name(Text(s.clone()))),
        Step::Entry(k, v) => Edit::SetMetadata(Metadata::Entry {
            key: format!("k{k}").into(),
            value: v.map(Value::Int),
        }),
        Step::Batch(steps) => {
            // Resolve each step against a scratch copy that has the earlier ones applied. An
            // edit that fails there stays in, so the whole batch must fail and roll back.
            let mut scratch = map.clone();
            let mut edits = Vec::new();
            for step in steps {
                let Some(edit) = resolve(step, &scratch) else {
                    continue;
                };
                let _ = edit.clone().apply(&mut scratch);
                edits.push(edit);
            }
            Edit::Batch(edits)
        }
    })
}

proptest! {
    #[test]
    fn undoing_every_edit_restores_the_map(steps in prop::collection::vec(step(), 0..40)) {
        let original = empty_map();
        let mut doc = MapDocument::new(original.clone(), defs());
        let mut applied = 0;
        let mut snapshots = vec![original.clone()];
        for step in &steps {
            let Some(edit) = resolve(step, doc.map()) else { continue };
            let before = doc.map().clone();
            if doc.apply(edit).is_ok() {
                applied += 1;
                snapshots.push(doc.map().clone());
            } else {
                // A failed edit, a batch that fails part way included, changes nothing.
                prop_assert_eq!(doc.map(), &before);
            }
        }
        // Undo walks back through every intermediate state, not only the first.
        for expected in snapshots.iter().rev().skip(1) {
            prop_assert!(doc.undo().unwrap());
            prop_assert_eq!(doc.map(), expected);
        }
        prop_assert_eq!(doc.map(), &original);
        prop_assert!(!doc.can_undo());
        // Redo replays forward to the same final state.
        for _ in 0..applied {
            prop_assert!(doc.redo().unwrap());
        }
        prop_assert_eq!(doc.map(), snapshots.last().unwrap());
    }
}

#[test]
fn board_refreshes_after_each_edit_and_undo() {
    let mut doc = MapDocument::new(empty_map(), defs());
    assert!(doc.board().is_ok());
    doc.apply(Edit::PlacePiece(piece(0, 0, 0, 0))).unwrap();
    let one = doc.board().unwrap().clone();
    // The same level and cell overlap, which the board reports without rejecting the edit.
    doc.apply(Edit::PlacePiece(piece(1, 0, 0, 0))).unwrap();
    assert!(doc.board().is_err());
    doc.undo().unwrap();
    assert_eq!(doc.board().unwrap(), &one);
}

#[test]
fn new_edit_clears_redo() {
    let mut doc = MapDocument::new(empty_map(), defs());
    doc.apply(Edit::PlacePiece(piece(0, 0, 0, 0))).unwrap();
    doc.undo().unwrap();
    assert!(doc.can_redo());
    doc.apply(Edit::PlacePiece(piece(3, 3, 0, 0))).unwrap();
    assert!(!doc.can_redo());
    assert!(!doc.redo().unwrap());
}

#[test]
fn bad_edits_fail_without_changing_the_map() {
    let mut doc = MapDocument::new(empty_map(), defs());
    assert_eq!(
        doc.apply(Edit::RemovePiece { index: 0 }),
        Err(EditError::NoSuchPiece(0))
    );
    assert_eq!(
        doc.apply(Edit::EraseZone {
            zone: "nope".into(),
            cells: Vec::new()
        }),
        Err(EditError::NoSuchZone("nope".into()))
    );
    assert_eq!(doc.map(), &empty_map());
    assert!(!doc.can_undo());
}

#[test]
fn erasing_the_last_cell_deletes_the_zone() {
    let mut doc = MapDocument::new(empty_map(), defs());
    let paint = Edit::PaintZone {
        zone: "start".into(),
        cells: vec![Hex::new(0, 0)],
    };
    doc.apply(paint).unwrap();
    doc.apply(Edit::EraseZone {
        zone: "start".into(),
        cells: vec![Hex::new(0, 0)],
    })
    .unwrap();
    assert!(doc.map().zones.is_empty());
    doc.undo().unwrap();
    assert_eq!(doc.map().zones.len(), 1);
}
