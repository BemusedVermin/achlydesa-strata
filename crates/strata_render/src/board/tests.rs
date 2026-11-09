use std::collections::BTreeMap;

use bevy::asset::AssetPlugin;
use strata_kernel::{DefId, Hex, Rotation, Shape, Sym};
use strata_model::board::{BoardDefs, compile};
use strata_model::{
    FeatureCell, FeatureDef, MapConstraints, MapDef, PlacedFeature, PlacedPiece, Segment,
    SourceLoc, Text, TileDef,
};
use strata_scene::{Orientation, SceneBoard};

use super::*;

struct Defs(TileDef, FeatureDef);

impl BoardDefs for Defs {
    fn tile(&self, _: &Sym) -> Option<&TileDef> {
        Some(&self.0)
    }
    fn feature(&self, _: &Sym) -> Option<&FeatureDef> {
        Some(&self.1)
    }
}

/// A slab with a crate on it at `q = 0` and another pair at `q = 20`, which is a different chunk.
fn board() -> SceneBoard {
    let tile = TileDef {
        id: DefId(0),
        name: Text::default(),
        terrain: DefId(1),
        thickness: 1,
        shape: Shape::new([Hex::ORIGIN]),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    let crate_def = FeatureDef {
        id: DefId(0),
        name: Text::default(),
        blocks_los: true,
        cells: vec![FeatureCell {
            at: Hex::ORIGIN,
            segments: vec![Segment {
                from: 0,
                to: 2,
                top: true,
            }],
        }],
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    let piece = |q| PlacedPiece {
        piece: "slab".into(),
        at: Hex::new(q, 0),
        rot: Rotation::from_steps(0),
        level: 0,
    };
    let feature = |q| PlacedFeature {
        feature: "crate".into(),
        at: Hex::new(q, 0),
        rot: Rotation::from_steps(0),
    };
    let map = MapDef {
        id: DefId(0),
        name: Text::default(),
        authors: Vec::new(),
        orientation: "pointy".into(),
        pieces: vec![piece(0), piece(20)],
        features: vec![feature(0), feature(20)],
        zones: Default::default(),
        relic_spots: Default::default(),
        constraints: MapConstraints {
            inventory: None,
            require_support: false,
        },
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    SceneBoard {
        board: compile(&map, &Defs(tile, crate_def)).unwrap(),
        orientation: Orientation::Pointy,
    }
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), BoardPlugin))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(ClientBoard(board()));
    app.update();
    app
}

fn chunks_drawn(app: &mut App) -> BTreeMap<(ChunkKey, Material), Entity> {
    let mut query = app.world_mut().query::<(Entity, &TerrainChunk)>();
    let all = query.iter(app.world());
    all.map(|(e, c)| ((c.key, c.material), e)).collect()
}

#[test]
fn the_board_is_drawn_one_mesh_per_chunk_and_material() {
    let mut app = app();
    // Two chunks, each with terrain and one feature.
    assert_eq!(chunks_drawn(&mut app).len(), 4);
}

#[test]
fn removing_a_feature_rebuilds_only_its_chunk() {
    let mut app = app();
    let before = chunks_drawn(&mut app);
    app.world_mut()
        .resource_mut::<RemovedFeatures>()
        .0
        .insert(0);
    app.update();
    let after = chunks_drawn(&mut app);
    assert_eq!(after.len(), 3);
    assert!(!after.keys().any(|(_, m)| *m == Material::Feature(0)));
    let far = |map: &BTreeMap<(ChunkKey, Material), Entity>| {
        map.iter()
            .filter(|((k, _), _)| k.q == 2)
            .map(|(k, e)| (*k, *e))
            .collect::<BTreeMap<_, _>>()
    };
    let near_terrain = |map: &BTreeMap<(ChunkKey, Material), Entity>| {
        map.iter()
            .find(|((k, m), _)| k.q == 0 && matches!(m, Material::Terrain(_)))
            .map(|(_, e)| *e)
    };
    assert_eq!(far(&before), far(&after), "the far chunk was untouched");
    assert_ne!(
        near_terrain(&before),
        near_terrain(&after),
        "the near chunk was rebuilt"
    );
}

#[test]
fn slicing_the_levels_rebuilds_everything_without_the_hidden_ones() {
    let mut app = app();
    app.world_mut().resource_mut::<LevelSlice>().hide_from = Some(1);
    app.update();
    let drawn = chunks_drawn(&mut app);
    assert_eq!(drawn.len(), 2, "only the slabs are left");
    assert!(drawn.keys().all(|(_, m)| matches!(m, Material::Terrain(_))));
}
