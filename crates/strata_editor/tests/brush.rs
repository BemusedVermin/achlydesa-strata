//! Brush shapes and the palette built from the content registry.

use std::path::PathBuf;

use proptest::prelude::*;
use strata_content::{ContentRegistry, load};
use strata_editor::{
    FILL_LIMIT, Occupancy, OpenedMap, Palette, PaletteEntry, PaletteItem, RegistryDefs, Stock,
    fill, line,
};
use strata_kernel::{Hex, Rotation};
use strata_model::{MapDef, PlacedPiece, Text};

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/maps"))
}

fn registry() -> ContentRegistry {
    load(&[root()]).unwrap()
}

fn opened(name: &str) -> OpenedMap {
    OpenedMap::open(&root().join(format!("t/maps/{name}.lua"))).unwrap()
}

fn hexes(cells: &[(i32, i32)]) -> Vec<Hex> {
    cells.iter().map(|&(q, r)| Hex::new(q, r)).collect()
}

fn hex() -> impl Strategy<Value = Hex> {
    (-50..50i32, -50..50i32).prop_map(|(q, r)| Hex::new(q, r))
}

proptest! {
    #[test]
    fn a_line_steps_one_hex_at_a_time_from_end_to_end(a in hex(), b in hex()) {
        let cells = line(a, b);
        prop_assert_eq!(cells.len() as u32, a.distance(b) + 1);
        prop_assert_eq!(cells.first(), Some(&a));
        prop_assert_eq!(cells.last(), Some(&b));
        for (i, cell) in cells.iter().enumerate() {
            // Each hex is one step further from the start and one closer to the end.
            prop_assert_eq!(a.distance(*cell), i as u32);
            prop_assert_eq!(b.distance(*cell), a.distance(b) - i as u32);
        }
    }

}

#[test]
fn a_line_to_itself_is_one_hex() {
    assert_eq!(line(Hex::new(2, -1), Hex::new(2, -1)), hexes(&[(2, -1)]));
}

#[test]
fn a_line_along_a_row_takes_the_row() {
    assert_eq!(
        line(Hex::new(0, 0), Hex::new(3, 0)),
        hexes(&[(0, 0), (1, 0), (2, 0), (3, 0)])
    );
}

#[test]
fn a_fill_on_open_ground_fills_the_hole_inside_the_terrain() {
    let opened = opened("fair");
    let defs = RegistryDefs::new(opened.registry.clone(), opened.pack.clone());
    let mut map = opened.map.clone();
    // Dig a hole in the west grass: the hex comes out, and the fill finds exactly it.
    map.pieces.retain(|p| p.at != Hex::new(1, 0));
    let occupancy = Occupancy::of(&map, &defs);
    assert_eq!(fill(&occupancy, Hex::new(1, 0)), hexes(&[(1, 0)]));
    // Outside the terrain's bounding box, a fill covers the hex under the cursor only.
    assert_eq!(fill(&occupancy, Hex::new(9, 9)), hexes(&[(9, 9)]));
    // On an empty map too.
    assert_eq!(
        fill(&Occupancy::default(), Hex::new(9, 9)),
        hexes(&[(9, 9)])
    );
}

#[test]
fn a_fill_stops_at_the_limit() {
    let opened = opened("fair");
    let defs = RegistryDefs::new(opened.registry.clone(), opened.pack.clone());
    let mut map = opened.map.clone();
    map.pieces.push(PlacedPiece {
        piece: "grass_1".into(),
        at: Hex::new(200, 200),
        rot: Rotation::default(),
        level: 0,
    });
    let occupancy = Occupancy::of(&map, &defs);
    assert_eq!(fill(&occupancy, Hex::new(100, 100)).len(), FILL_LIMIT);
}

#[test]
fn the_palette_lists_every_tile_then_every_feature_of_the_loaded_packs() {
    let registry = registry();
    let palette = Palette::new(&RegistryDefs::new(&registry, "t"));
    let entry = |item: PaletteItem, full: &str, label: &str, cells, thickness| PaletteEntry {
        item,
        full_id: full.into(),
        label: Text(label.into()),
        cells,
        thickness,
    };
    let tile = |name: &str| PaletteItem::Tile(name.into());
    let feature = |name: &str| PaletteItem::Feature(name.into());
    assert_eq!(
        palette.entries,
        vec![
            entry(tile("grass_1"), "t:grass_1", "Grass", 1, 1),
            entry(tile("grass_2"), "t:grass_2", "Grass pair", 2, 1),
            entry(tile("hill_2"), "t:hill_2", "Hill", 1, 2),
            entry(tile("water_1"), "t:water_1", "Water", 1, 1),
            entry(feature("tree"), "t:tree", "Tree", 1, 0),
            entry(feature("wall"), "t:wall", "Wall", 2, 0),
        ]
    );
    assert_eq!(palette.entry(&tile("hill_2")).map(|e| e.thickness), Some(2));
    assert_eq!(palette.entry(&tile("nothing")), None);
}

#[test]
fn the_stock_is_the_inventory_less_what_the_map_uses() {
    let opened = opened("inventory");
    let defs = RegistryDefs::new(opened.registry.clone(), opened.pack.clone());
    let stock = Stock::of(&opened.map, &defs).unwrap();
    let remaining = |item: PaletteItem| stock.remaining(&item, &defs);
    // Three grass in the box, four used under two spellings.
    assert_eq!(remaining(PaletteItem::Tile("grass_1".into())), -1);
    assert_eq!(remaining(PaletteItem::Tile("t:grass_1".into())), -1);
    assert_eq!(remaining(PaletteItem::Tile("water_1".into())), 0);
    // The box holds no hills, and the map uses one.
    assert_eq!(remaining(PaletteItem::Tile("hill_2".into())), -1);
    assert_eq!(remaining(PaletteItem::Tile("grass_2".into())), 0);
    assert_eq!(remaining(PaletteItem::Feature("tree".into())), -1);
    assert_eq!(remaining(PaletteItem::Tile("nothing".into())), 0);
}

#[test]
fn a_map_without_an_inventory_has_no_stock() {
    let opened = opened("fair");
    let defs = RegistryDefs::new(opened.registry.clone(), opened.pack.clone());
    assert_eq!(Stock::of(&opened.map, &defs), None);
    let mut map: MapDef = opened.map.clone();
    map.constraints.inventory = Some("missing".into());
    assert_eq!(Stock::of(&map, &defs), None);
}
