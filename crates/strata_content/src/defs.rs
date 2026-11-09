//! Resolving the names a pack's maps refer to against a loaded registry.

use std::borrow::Borrow;

use strata_kernel::{DefId, Sym};
use strata_lua::Def;
use strata_model::board::BoardDefs;
use strata_model::{FeatureDef, TileDef};

use crate::registry::ContentRegistry;

/// Resolves a map's names against a registry, as the map's own pack sees them.
///
/// The registry and pack may be owned or borrowed, so one type serves both a long-lived editor
/// document and a one-off compile.
#[derive(Clone, Debug)]
pub struct RegistryDefs<R = ContentRegistry, P = String> {
    registry: R,
    pack: P,
}

impl<R: Borrow<ContentRegistry>, P: AsRef<str>> RegistryDefs<R, P> {
    /// Resolves names for content authored in `pack`.
    pub fn new(registry: R, pack: P) -> Self {
        Self { registry, pack }
    }

    /// Returns the registry names are resolved against.
    pub fn registry(&self) -> &ContentRegistry {
        self.registry.borrow()
    }

    /// Returns the pack names are resolved from.
    pub fn pack(&self) -> &str {
        self.pack.as_ref()
    }

    /// Returns the `T` that `name` refers to, as [`ContentRegistry::lookup`] resolves it.
    pub fn lookup<T: Def>(&self, name: &Sym) -> Option<(DefId, &T)> {
        self.registry()
            .lookup::<T>(self.pack.as_ref(), name.as_str())
    }
}

impl<R: Borrow<ContentRegistry>, P: AsRef<str>> BoardDefs for RegistryDefs<R, P> {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        self.lookup::<TileDef>(name).map(|(_, def)| def)
    }

    fn feature(&self, name: &Sym) -> Option<&FeatureDef> {
        self.lookup::<FeatureDef>(name).map(|(_, def)| def)
    }
}
