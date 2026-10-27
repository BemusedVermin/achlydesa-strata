//! Round-trips each definition type through its hand-written JSON fixture.

use std::fmt::Debug;

use serde::Serialize;
use serde::de::DeserializeOwned;
use strata_kernel::{StableHash, StableHasher};
use strata_model::*;

fn hash<T: StableHash>(value: &T) -> u64 {
    let mut hasher = StableHasher::new();
    value.stable_hash(&mut hasher);
    hasher.finish()
}

fn round_trip<T>(json: &str) -> T
where
    T: Serialize + DeserializeOwned + StableHash + Clone + Debug + PartialEq,
{
    let first: T = serde_json::from_str(json).expect("fixture parses");
    let again: T = serde_json::from_str(&serde_json::to_string(&first).unwrap()).unwrap();
    assert_eq!(first, again);
    assert_eq!(hash(&first), hash(&again));
    assert_eq!(hash(&first), hash(&first.clone()));
    first
}

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("fixtures/", $name, ".json"))
    };
}

#[test]
fn stat() {
    let def: StatDef = round_trip(fixture!("stat"));
    assert_eq!((def.min, def.max, def.order), (0, 99, 5));
}

#[test]
fn die_with_faces_and_range() {
    let faces: DieDef = round_trip(fixture!("die_faces"));
    assert!(matches!(&faces.kind, DieKind::Faces(f) if f.len() == 6));
    let range: DieDef = round_trip(fixture!("die_range"));
    assert_eq!(range.kind, DieKind::Range(1, 20));
}

#[test]
fn marker() {
    let def: MarkerDef = round_trip(fixture!("marker"));
    assert_eq!(def.stacking.as_str(), "count");
}

#[test]
fn traits() {
    let def: TraitsDef = round_trip(fixture!("traits"));
    assert_eq!(def.keys.len(), 2);
}

#[test]
fn faction() {
    let def: FactionDef = round_trip(fixture!("faction"));
    assert_eq!(def.color, "#B5893A");
}

#[test]
fn terrain() {
    let def: TerrainDef = round_trip(fixture!("terrain"));
    assert_eq!(def.abilities.len(), 1);
}

#[test]
fn tile() {
    let def: TileDef = round_trip(fixture!("tile"));
    assert_eq!((def.thickness, def.shape.len()), (1, 2));
}

#[test]
fn feature() {
    let def: FeatureDef = round_trip(fixture!("feature"));
    assert!(def.blocks_los);
    assert!(!def.cells[0].segments[0].top);
    assert!(def.cells[1].segments[0].top);
}

#[test]
fn footprint() {
    let def: FootprintDef = round_trip(fixture!("footprint"));
    assert_eq!((def.move_mode, def.max_spread), (MoveMode::Snake, 1));
}

#[test]
fn map() {
    let def: MapDef = round_trip(fixture!("map"));
    assert_eq!(def.pieces.len(), 2);
    assert!(def.constraints.require_support);
}

#[test]
fn inventory() {
    let def: InventoryDef = round_trip(fixture!("inventory"));
    assert_eq!(def.tiles.len(), 2);
}

#[test]
fn hash_depends_on_content() {
    let a: StatDef = serde_json::from_str(fixture!("stat")).unwrap();
    let mut b = a.clone();
    b.max = 98;
    assert_ne!(hash(&a), hash(&b));
}
