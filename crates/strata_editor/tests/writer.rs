use strata_editor::{WriteError, write_map};
use strata_kernel::{DefId, Hex, PackId, Rotation, Value};
use strata_lua::{DataStage, FrozenDef, PackSource, freeze};
use strata_model::{MapConstraints, MapDef, PlacedFeature, PlacedPiece, SourceLoc, Text};

fn piece(name: &str, q: i32, r: i32, rot: i32, level: u8) -> PlacedPiece {
    PlacedPiece {
        piece: name.into(),
        at: Hex::new(q, r),
        rot: Rotation::from_steps(rot),
        level,
    }
}

fn empty() -> MapDef {
    MapDef {
        id: DefId(0),
        name: Text("Empty".into()),
        authors: Vec::new(),
        orientation: "flat".into(),
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

fn twin_fords() -> MapDef {
    let mut map = empty();
    map.name = Text("Twin Fords".into());
    map.authors = vec!["test".into(), "other".into()];
    map.orientation = "pointy".into();
    map.pieces = vec![
        piece("grass_24", 0, 0, 0, 0),
        piece("rock_3", 4, -1, 2, 1),
        piece("water_1", 6, 2, 0, 1),
    ];
    map.features = vec![PlacedFeature {
        feature: "ruin_arch".into(),
        at: Hex::new(5, 1),
        rot: Rotation::from_steps(0),
    }];
    map.zones.insert(
        "start_b".into(),
        vec![Hex::new(7, -3), Hex::new(7, -2), Hex::new(6, -2)],
    );
    map.zones.insert(
        "start_a".into(),
        vec![Hex::new(-6, 2), Hex::new(-6, 3), Hex::new(-5, 2)],
    );
    map.zones.insert("end".into(), vec![Hex::new(0, 1)]);
    map.relic_spots.insert("center".into(), Hex::new(1, 0));
    map.constraints = MapConstraints {
        inventory: Some("starter_box".into()),
        require_support: true,
    };
    map.tags.insert("duel".into());
    map.meta
        .insert("note".into(), Value::Str("a \"quoted\"\nline\\".into()));
    map.meta.insert(
        "nested".into(),
        Value::List(vec![Value::Int(-3), Value::Bool(true)]),
    );
    map
}

/// Writes `source` as `maps/m.lua` of a pack, runs the data stage and freezes the map record.
fn load(source: &str) -> MapDef {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("p");
    std::fs::create_dir_all(root.join("maps")).unwrap();
    std::fs::write(root.join("maps/m.lua"), source).unwrap();
    std::fs::write(root.join("data.lua"), "require('maps.m')").unwrap();
    let pack = PackSource {
        id: PackId::new("p"),
        root,
        dependencies: Vec::new(),
    };
    let records = freeze(&DataStage::run(&[pack]).unwrap()).unwrap();
    match &records[0].def {
        FrozenDef::Map(map) => map.clone(),
        other => panic!("not a map: {other:?}"),
    }
}

/// `map` as the loader returns it: the ID is assigned by the registry and the source is the file.
fn loaded_like(map: &MapDef, loaded: &MapDef) -> MapDef {
    MapDef {
        id: loaded.id,
        source: loaded.source.clone(),
        ..map.clone()
    }
}

#[test]
fn writing_twice_is_byte_identical() {
    let map = twin_fords();
    let first = write_map(&map, "twin_fords").unwrap();
    assert_eq!(first, write_map(&map, "twin_fords").unwrap());
    assert_eq!(first, write_map(&map.clone(), "twin_fords").unwrap());
}

#[test]
fn output_matches_the_design_style() {
    insta::assert_snapshot!(write_map(&twin_fords(), "twin_fords").unwrap());
    insta::assert_snapshot!("empty", write_map(&empty(), "empty").unwrap());
}

#[test]
fn written_maps_load_through_the_data_stage() {
    let map = twin_fords();
    let loaded = load(&write_map(&map, "twin_fords").unwrap());
    assert_eq!(loaded, loaded_like(&map, &loaded));
}

#[test]
fn empty_and_awkward_names_load() {
    let mut map = empty();
    map.zones.insert("end".into(), vec![Hex::new(0, 0)]);
    map.zones.insert("two words".into(), vec![Hex::new(1, 1)]);
    map.zones.insert("9lives".into(), Vec::new());
    let loaded = load(&write_map(&map, "empty").unwrap());
    assert_eq!(loaded, loaded_like(&map, &loaded));
    let empty = load(&write_map(&self::empty(), "empty").unwrap());
    assert_eq!(empty, loaded_like(&self::empty(), &empty));
}

#[test]
fn handles_in_meta_are_rejected() {
    let mut map = empty();
    map.meta.insert("h".into(), Value::Id(DefId(3)));
    assert_eq!(
        write_map(&map, "m"),
        Err(WriteError::UnsupportedMeta("h".into()))
    );
}
