use strata_kernel::Hex;

use super::*;
use crate::layout::Orientation;
use crate::tests::{GRASS, ROCK, board};

const TERRAIN_GRASS: Material = Material::Terrain(GRASS);
const TERRAIN_ROCK: Material = Material::Terrain(ROCK);

fn opts(hide_from: Option<u32>) -> MeshOptions<'static> {
    MeshOptions {
        hide_from,
        removed: None,
    }
}

fn all(board: &SceneBoard, hide: Option<u32>) -> BTreeMap<Material, MeshData> {
    let mut out: BTreeMap<Material, MeshData> = BTreeMap::new();
    for chunk in chunks(board) {
        for (terrain, mesh) in chunk_meshes(board, chunk, opts(hide)) {
            let into = out.entry(terrain).or_default();
            let base = into.positions.len() as u32;
            into.positions.extend(mesh.positions);
            into.normals.extend(mesh.normals);
            into.uvs.extend(mesh.uvs);
            into.indices.extend(mesh.indices.iter().map(|i| i + base));
        }
    }
    out
}

/// Counts triangles whose normal is `axis`-aligned upward, downward or sideways.
fn count(mesh: &MeshData, keep: impl Fn(Vec3) -> bool) -> usize {
    mesh.indices
        .chunks(3)
        .filter(|t| keep(Vec3::from(mesh.normals[t[0] as usize])))
        .count()
}

fn hex_only(board: &SceneBoard, hex: Hex) -> BTreeMap<Material, MeshData> {
    let mut out = BTreeMap::new();
    for (t, m) in chunk_meshes(board, chunk_of(hex), MeshOptions::default()) {
        let mut keep = MeshData::default();
        for tri in m.indices.chunks(3) {
            let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let center = (Vec3::from(m.positions[a])
                + Vec3::from(m.positions[b])
                + Vec3::from(m.positions[c]))
                / 3.0;
            let (x, z) = board.orientation.center(hex);
            if (center.x - x).abs() < 1.01 && (center.z - z).abs() < 1.01 {
                let base = keep.positions.len() as u32;
                for i in tri {
                    keep.positions.push(m.positions[*i as usize]);
                    keep.normals.push(m.normals[*i as usize]);
                    keep.uvs.push(m.uvs[*i as usize]);
                }
                keep.indices.extend([base, base + 1, base + 2]);
            }
        }
        out.insert(t, keep);
    }
    out
}

#[test]
fn lone_slab_has_top_bottom_skipped_and_six_walls() {
    for o in [Orientation::Pointy, Orientation::Flat] {
        let b = board(o);
        let mesh = &hex_only(&b, Hex::new(0, 0))[&TERRAIN_GRASS];
        // Sitting on the ground: no underside. Top is 4 triangles, six walls are 12.
        assert_eq!(count(mesh, |n| n.y > 0.5), 4, "{o:?}");
        assert_eq!(count(mesh, |n| n.y < -0.5), 0, "{o:?}");
        assert_eq!(count(mesh, |n| n.y.abs() < 0.5), 12, "{o:?}");
    }
}

#[test]
fn faces_are_wound_outward() {
    let b = board(Orientation::Pointy);
    for mesh in all(&b, None).values() {
        for t in mesh.indices.chunks(3) {
            let p = |i: u32| Vec3::from(mesh.positions[i as usize]);
            let face = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            assert!(face.dot(Vec3::from(mesh.normals[t[0] as usize])) > 0.0);
        }
    }
}

#[test]
fn stacked_segments_hide_the_shared_face() {
    let b = board(Orientation::Pointy);
    // (6, 0): rock levels 0..3 under grass 3..4. Rock loses its top, grass its underside.
    let rock = &hex_only(&b, Hex::new(6, 0))[&TERRAIN_ROCK];
    let grass = &hex_only(&b, Hex::new(6, 0))[&TERRAIN_GRASS];
    assert_eq!(count(rock, |n| n.y > 0.5), 0);
    assert_eq!(count(grass, |n| n.y < -0.5), 0);
    assert_eq!(count(grass, |n| n.y > 0.5), 4);
}

#[test]
fn walls_shared_with_a_neighbor_are_not_drawn() {
    let b = board(Orientation::Pointy);
    // The slab at (0, 3) touches slabs at (-1, 3) and (1, 3): two fewer walls than a lone slab.
    let mesh = &hex_only(&b, Hex::new(0, 3))[&TERRAIN_GRASS];
    assert_eq!(count(mesh, |n| n.y.abs() < 0.5), 8);
}

#[test]
fn bridge_keeps_its_gap_and_draws_its_underside() {
    let b = board(Orientation::Pointy);
    let meshes = hex_only(&b, Hex::new(0, 3));
    let deck = &meshes[&TERRAIN_ROCK];
    // The deck is one level thick at level 4 over a slab at level 0: levels 1..4 are open.
    assert_eq!(count(deck, |n| n.y < -0.5), 4);
    assert!(
        deck.positions
            .iter()
            .all(|p| p[1] >= 4.0 * LEVEL_HEIGHT - 1e-4)
    );
    // The slab's top face is drawn because nothing sits on it.
    assert_eq!(count(&meshes[&TERRAIN_GRASS], |n| n.y > 0.5), 4);
}

#[test]
fn slicing_hides_levels_and_exposes_what_is_below() {
    let b = board(Orientation::Pointy);
    let full = all(&b, None);
    let sliced = all(&b, Some(1));
    assert!(
        sliced
            .get(&TERRAIN_ROCK)
            .is_some_and(|m| m.positions.iter().all(|p| p[1] <= LEVEL_HEIGHT + 1e-4))
    );
    assert!(sliced[&TERRAIN_ROCK].triangles() < full[&TERRAIN_ROCK].triangles());
    assert!(all(&b, Some(0)).is_empty());
}

#[test]
fn chunks_cover_the_board_and_meshing_is_deterministic() {
    let b = board(Orientation::Flat);
    assert!(chunks(&b).len() >= 2);
    assert_eq!(all(&b, None), all(&b, None));
    assert_eq!(chunk_of(Hex::new(-1, 0)), ChunkKey { q: -1, r: 0 });
}

/// The slab at `(0, 0)` with a two-level crate standing on it as feature 0, and a lone crate
/// at `(3, 0)` as feature 1.
fn board_with_features() -> SceneBoard {
    use strata_kernel::{Rotation, Shape, Sym};
    use strata_model::board::{BoardDefs, compile};
    use strata_model::{
        FeatureCell, FeatureDef, MapConstraints, MapDef, PlacedFeature, PlacedPiece, Segment,
        SourceLoc, Text, TileDef,
    };

    struct Defs(TileDef, FeatureDef);
    impl BoardDefs for Defs {
        fn tile(&self, _: &Sym) -> Option<&TileDef> {
            Some(&self.0)
        }
        fn feature(&self, _: &Sym) -> Option<&FeatureDef> {
            Some(&self.1)
        }
    }
    let tile = TileDef {
        id: DefId(0),
        name: Text::default(),
        terrain: GRASS,
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
        pieces: vec![piece(0), piece(3)],
        features: vec![feature(0), feature(3)],
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

#[test]
fn a_feature_is_drawn_on_top_of_its_terrain_and_hides_the_shared_face() {
    let b = board_with_features();
    let meshes = hex_only(&b, Hex::new(0, 0));
    let slab = &meshes[&TERRAIN_GRASS];
    let crate_mesh = &meshes[&Material::Feature(0)];
    assert_eq!(count(slab, |n| n.y > 0.5), 0);
    assert_eq!(count(crate_mesh, |n| n.y < -0.5), 0);
    assert_eq!(count(crate_mesh, |n| n.y > 0.5), 4);
    assert_eq!(count(crate_mesh, |n| n.y.abs() < 0.5), 12);
}

#[test]
fn removing_a_feature_uncovers_the_terrain_and_only_dirties_its_chunk() {
    let b = board_with_features();
    let removed: BTreeSet<u32> = [0].into();
    let options = MeshOptions {
        hide_from: None,
        removed: Some(&removed),
    };
    let meshes = chunk_meshes(&b, chunk_of(Hex::ORIGIN), options);
    assert!(!meshes.contains_key(&Material::Feature(0)));
    assert!(meshes.contains_key(&Material::Feature(1)));
    let slab = Vec::from_iter(&meshes[&TERRAIN_GRASS].normals);
    assert!(slab.iter().any(|n| n[1] > 0.5));
    assert_eq!(
        feature_chunks(&b, &removed),
        BTreeSet::from([chunk_of(Hex::ORIGIN)])
    );
    assert!(feature_chunks(&b, &BTreeSet::new()).is_empty());
}
