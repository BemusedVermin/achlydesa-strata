//! Typed identifiers for runtime state (design §12.1).

use serde::{Deserialize, Serialize};
use strata_kernel::{ModuleId, StableHash, Sym};

macro_rules! index_id {
    ($($(#[$doc:meta])* $name:ident($inner:ty)),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(
            Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
            Serialize, Deserialize, StableHash,
        )]
        pub struct $name(pub $inner);

        impl $name {
            /// Position of this ID in its state vector.
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    )*};
}

index_id!(
    /// Index of a figure in `GameState::figures`.
    FigureId(u32),
    /// Index of a unit in `GameState::units`.
    UnitId(u32),
    /// Index of a player in `GameState::players`.
    PlayerId(u8),
    /// Index of a relic in `GameState::relics`.
    RelicId(u32),
);

/// The namespaced name of a module or ability variable.
#[derive(
    Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, StableHash,
)]
pub struct VarKey {
    /// The module that owns the variable.
    pub module: ModuleId,
    /// The variable's name within the module.
    pub name: Sym,
}
