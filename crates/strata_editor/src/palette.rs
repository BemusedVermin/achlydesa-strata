//! The palette: every tile and feature the loaded packs define, and how many of each the map's
//! inventory has left (design §14.2).

use std::borrow::Borrow;
use std::collections::BTreeMap;

use strata_content::{ContentRegistry, RegistryDefs};
use strata_kernel::{DefId, Sym};
use strata_model::{FeatureDef, MapDef, Text, TileDef};

use crate::defs::EditorDefs;
use crate::tally::{Count, Key, tally};

/// Something the editor can place: a tile or a feature, by the name a map writes for it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PaletteItem {
    /// A tile, placed as a piece.
    Tile(Sym),
    /// A feature.
    Feature(Sym),
}

impl PaletteItem {
    /// The name a map writes for the item.
    pub fn name(&self) -> &Sym {
        match self {
            Self::Tile(name) | Self::Feature(name) => name,
        }
    }

    /// The item's definition handle, or `None` if its name does not resolve.
    pub fn id(&self, defs: &impl EditorDefs) -> Option<DefId> {
        match self {
            Self::Tile(name) => defs.tile_id(name),
            Self::Feature(name) => defs.feature_id(name),
        }
    }
}

/// One palette button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteEntry {
    /// What the button places.
    pub item: PaletteItem,
    /// The definition's full `pack:name` ID.
    pub full_id: String,
    /// Display name.
    pub label: Text,
    /// How many hexes the item covers.
    pub cells: usize,
    /// A tile's thickness in levels; 0 for a feature.
    pub thickness: u8,
}

/// Every tile, then every feature, of the loaded packs, each in ID order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Palette {
    /// The buttons, tiles first.
    pub entries: Vec<PaletteEntry>,
}

impl Palette {
    /// Builds the palette from the registry `defs` resolves against.
    ///
    /// An item of the map's own pack is named the short way (`grass_1`) when that name resolves
    /// back to it. Every other item is named by its full ID (`base:grass_1`): a short name that
    /// only resolves by searching every pack could bind to another pack's item of the same name
    /// once more packs are loaded, so the map would no longer mean what was picked.
    pub fn new<R: Borrow<ContentRegistry>, P: AsRef<str>>(defs: &RegistryDefs<R, P>) -> Self {
        let registry = defs.registry();
        let pack = defs.pack();
        let tiles = registry.iter::<TileDef>().map(|(id, full, tile)| {
            let name = map_name(full, pack, |n| {
                defs.lookup::<TileDef>(n).map(|(i, _)| i) == Some(id)
            });
            PaletteEntry {
                item: PaletteItem::Tile(name),
                full_id: full.to_owned(),
                label: tile.name.clone(),
                cells: tile.shape.len(),
                thickness: tile.thickness,
            }
        });
        let features = registry.iter::<FeatureDef>().map(|(id, full, feature)| {
            let name = map_name(full, pack, |n| {
                defs.lookup::<FeatureDef>(n).map(|(i, _)| i) == Some(id)
            });
            PaletteEntry {
                item: PaletteItem::Feature(name),
                full_id: full.to_owned(),
                label: feature.name.clone(),
                cells: feature.cells.len(),
                thickness: 0,
            }
        });
        Self {
            entries: tiles.chain(features).collect(),
        }
    }

    /// The entry that places `item`.
    pub fn entry(&self, item: &PaletteItem) -> Option<&PaletteEntry> {
        self.entries.iter().find(|e| &e.item == item)
    }
}

/// The name after the `pack:` prefix if `full` is in `pack` and `resolves` accepts it, and the
/// full ID otherwise.
fn map_name(full: &str, pack: &str, resolves: impl Fn(&Sym) -> bool) -> Sym {
    let short = full
        .strip_prefix(pack)
        .and_then(|rest| rest.strip_prefix(':'));
    match short.map(Sym::new) {
        Some(short) if resolves(&short) => short,
        _ => Sym::new(full),
    }
}

/// How many of each tile and feature the map's inventory has left.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stock {
    tiles: BTreeMap<DefId, i64>,
    features: BTreeMap<DefId, i64>,
}

impl Stock {
    /// The stock left after `map`'s pieces and features, or `None` when the map sets no
    /// inventory or names one that does not exist.
    ///
    /// Names are compared by the definitions they resolve to, as validation does; names that do
    /// not resolve count against nothing.
    pub fn of(map: &MapDef, defs: &impl EditorDefs) -> Option<Self> {
        let inventory = defs.inventory(map.constraints.inventory.as_ref()?)?;
        let pieces = map.pieces.iter().map(|p| (&p.piece, p.at));
        let features = map.features.iter().map(|f| (&f.feature, f.at));
        Some(Self {
            tiles: left(tally(pieces, &inventory.tiles, |n| defs.tile_id(n))),
            features: left(tally(features, &inventory.features, |n| defs.feature_id(n))),
        })
    }

    /// How many of `item` are left; negative when the map already uses more than the inventory
    /// holds, and 0 for an item it does not hold or a name that does not resolve.
    pub fn remaining(&self, item: &PaletteItem, defs: &impl EditorDefs) -> i64 {
        let counts = match item {
            PaletteItem::Tile(_) => &self.tiles,
            PaletteItem::Feature(_) => &self.features,
        };
        item.id(defs)
            .and_then(|id| counts.get(&id).copied())
            .unwrap_or(0)
    }
}

/// What is left of each resolved definition once its uses are taken from what the inventory
/// holds.
fn left(counts: BTreeMap<Key<'_>, Count<'_>>) -> BTreeMap<DefId, i64> {
    let resolved = counts.into_iter().filter_map(|(key, count)| {
        let id = key.ok()?;
        Some((id, i64::from(count.available) - count.uses.len() as i64))
    });
    resolved.collect()
}
