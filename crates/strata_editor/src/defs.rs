//! The definitions validation needs beyond the board's tiles and features.

use strata_content::RegistryDefs;
use strata_kernel::{DefId, Sym};
use strata_model::board::BoardDefs;
use strata_model::{FeatureDef, InventoryDef, TerrainDef, TileDef};

/// Resolves everything a map refers to: tiles and features for the board, plus the terrain
/// under each surface and the inventory the map is built from.
pub trait EditorDefs: BoardDefs {
    /// Returns the terrain with this handle.
    fn terrain(&self, id: DefId) -> Option<&TerrainDef>;
    /// Returns the inventory called `name`.
    fn inventory(&self, name: &Sym) -> Option<&InventoryDef>;
    /// Returns the handle of the tile called `name`, so different spellings count as one tile.
    fn tile_id(&self, name: &Sym) -> Option<DefId>;
    /// Returns the handle of the feature called `name`.
    fn feature_id(&self, name: &Sym) -> Option<DefId>;
}

impl EditorDefs for RegistryDefs {
    fn terrain(&self, id: DefId) -> Option<&TerrainDef> {
        self.registry().get::<TerrainDef>(id)
    }

    fn inventory(&self, name: &Sym) -> Option<&InventoryDef> {
        self.lookup::<InventoryDef>(name).map(|(_, def)| def)
    }

    fn tile_id(&self, name: &Sym) -> Option<DefId> {
        self.lookup::<TileDef>(name).map(|(id, _)| id)
    }

    fn feature_id(&self, name: &Sym) -> Option<DefId> {
        self.lookup::<FeatureDef>(name).map(|(id, _)| id)
    }
}
