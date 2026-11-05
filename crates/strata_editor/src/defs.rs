//! The definitions validation needs beyond the board's tiles and features.

use strata_content::ContentRegistry;
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
}

/// Resolves a map's names against a loaded pack set, as the map's own pack sees them.
#[derive(Clone, Debug)]
pub struct RegistryDefs {
    registry: ContentRegistry,
    pack: String,
}

impl RegistryDefs {
    /// Resolves names for a map authored in `pack`.
    pub fn new(registry: ContentRegistry, pack: impl Into<String>) -> Self {
        Self {
            registry,
            pack: pack.into(),
        }
    }
}

impl BoardDefs for RegistryDefs {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        let found = self.registry.lookup::<TileDef>(&self.pack, name.as_str());
        found.map(|(_, def)| def)
    }

    fn feature(&self, name: &Sym) -> Option<&FeatureDef> {
        let found = self
            .registry
            .lookup::<FeatureDef>(&self.pack, name.as_str());
        found.map(|(_, def)| def)
    }
}

impl EditorDefs for RegistryDefs {
    fn terrain(&self, id: DefId) -> Option<&TerrainDef> {
        self.registry.get::<TerrainDef>(id)
    }

    fn inventory(&self, name: &Sym) -> Option<&InventoryDef> {
        let found = self
            .registry
            .lookup::<InventoryDef>(&self.pack, name.as_str());
        found.map(|(_, def)| def)
    }
}
