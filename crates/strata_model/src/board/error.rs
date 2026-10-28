//! Errors raised while compiling a map into a board.

use strata_kernel::{Hex, Sym};
use thiserror::Error;

/// Why a map failed to compile. Indices refer to `MapDef::pieces` or `MapDef::features`.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum BoardError {
    /// A placed piece names a tile that does not exist.
    #[error("piece {piece} at {at:?} names unknown tile `{name}`")]
    UnknownTile {
        /// Index of the piece.
        piece: usize,
        /// The piece's anchor.
        at: Hex,
        /// The missing tile name.
        name: Sym,
    },
    /// A placed feature names a feature that does not exist.
    #[error("feature {feature} at {at:?} names unknown feature `{name}`")]
    UnknownFeature {
        /// Index of the feature.
        feature: usize,
        /// The feature's anchor.
        at: Hex,
        /// The missing feature name.
        name: Sym,
    },
    /// Two pieces occupy the same level range of one column.
    #[error("piece {piece} overlaps piece {other} at {hex:?}")]
    Overlap {
        /// Index of the piece being placed.
        piece: usize,
        /// Index of the piece already there.
        other: usize,
        /// The shared column.
        hex: Hex,
    },
    /// A raised piece cell has nothing directly beneath it.
    #[error("piece {piece} is unsupported at {hex:?}")]
    Unsupported {
        /// Index of the piece.
        piece: usize,
        /// The unsupported column.
        hex: Hex,
    },
    /// A feature cell lies on a column that has no terrain.
    #[error("feature {feature} is off the board at {hex:?}")]
    FeatureOffBoard {
        /// Index of the feature.
        feature: usize,
        /// The column without terrain.
        hex: Hex,
    },
}
