use strata_kernel::{DefId, Hex};

use super::*;
use crate::layout::Orientation;
use crate::tests::{GRASS, ROCK, board};

fn all(board: &SceneBoard, hide: Option<u32>) -> BTreeMap<DefId, MeshData> {
    let mut out: BTreeMap<DefId, MeshData> = BTreeMap::new();
    for chunk in chunks(board) {
        for (terrain, mesh) in chunk_meshes(board, chunk, hide) {
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

fn hex_only(board: &SceneBoard, hex: Hex) -> BTreeMap<DefId, MeshData> {
    let mut out = BTreeMap::new();
    for (t, m) in chunk_meshes(board, chunk_of(hex), None) {
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
        let mesh = &hex_only(&b, Hex::new(0, 0))[&GRASS];
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
    let rock = &hex_only(&b, Hex::new(6, 0))[&ROCK];
    let grass = &hex_only(&b, Hex::new(6, 0))[&GRASS];
    assert_eq!(count(rock, |n| n.y > 0.5), 0);
    assert_eq!(count(grass, |n| n.y < -0.5), 0);
    assert_eq!(count(grass, |n| n.y > 0.5), 4);
}

#[test]
fn walls_shared_with_a_neighbor_are_not_drawn() {
    let b = board(Orientation::Pointy);
    // The slab at (0, 3) touches slabs at (-1, 3) and (1, 3): two fewer walls than a lone slab.
    let mesh = &hex_only(&b, Hex::new(0, 3))[&GRASS];
    assert_eq!(count(mesh, |n| n.y.abs() < 0.5), 8);
}

#[test]
fn bridge_keeps_its_gap_and_draws_its_underside() {
    let b = board(Orientation::Pointy);
    let meshes = hex_only(&b, Hex::new(0, 3));
    let deck = &meshes[&ROCK];
    // The deck is one level thick at level 4 over a slab at level 0: levels 1..4 are open.
    assert_eq!(count(deck, |n| n.y < -0.5), 4);
    assert!(
        deck.positions
            .iter()
            .all(|p| p[1] >= 4.0 * LEVEL_HEIGHT - 1e-4)
    );
    // The slab's top face is drawn because nothing sits on it.
    assert_eq!(count(&meshes[&GRASS], |n| n.y > 0.5), 4);
}

#[test]
fn slicing_hides_levels_and_exposes_what_is_below() {
    let b = board(Orientation::Pointy);
    let full = all(&b, None);
    let sliced = all(&b, Some(1));
    assert!(
        sliced
            .get(&ROCK)
            .is_some_and(|m| m.positions.iter().all(|p| p[1] <= LEVEL_HEIGHT + 1e-4))
    );
    assert!(sliced[&ROCK].triangles() < full[&ROCK].triangles());
    assert!(all(&b, Some(0)).is_empty());
}

#[test]
fn chunks_cover_the_board_and_meshing_is_deterministic() {
    let b = board(Orientation::Flat);
    assert!(chunks(&b).len() >= 2);
    assert_eq!(all(&b, None), all(&b, None));
    assert_eq!(chunk_of(Hex::new(-1, 0)), ChunkKey { q: -1, r: 0 });
}
