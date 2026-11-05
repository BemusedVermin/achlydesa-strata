//! Validation findings: what is wrong with a map, how badly, and where (design §14.1).

use std::fmt;

use strata_kernel::{Hex, Sym};
use strata_model::board::BoardError;

/// How serious a finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The map cannot be played as it stands.
    Error,
    /// The map loads and plays, but probably not as the author meant.
    Warning,
}

/// What a finding is about. Piece and feature numbers index `MapDef::pieces` and `features`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindingKind {
    /// A piece names a tile that does not exist.
    UnknownTile {
        /// The piece.
        piece: usize,
        /// The missing tile name.
        name: Sym,
    },
    /// A placed feature names a feature that does not exist.
    UnknownFeature {
        /// The feature.
        feature: usize,
        /// The missing feature name.
        name: Sym,
    },
    /// Feature cells stand on columns with no terrain.
    FeatureOffBoard {
        /// The feature.
        feature: usize,
    },
    /// Two pieces share a level range in some columns.
    Overlap {
        /// The later piece.
        piece: usize,
        /// The earlier piece.
        other: usize,
    },
    /// A raised piece has cells with nothing directly beneath them.
    Unsupported {
        /// The piece.
        piece: usize,
    },
    /// The map's inventory does not exist.
    UnknownInventory {
        /// The missing inventory name.
        name: Sym,
    },
    /// The map uses more of a tile or feature than its inventory holds.
    InventoryOverrun {
        /// The tile or feature name.
        name: Sym,
        /// How many the map places.
        used: u32,
        /// How many the inventory holds.
        available: u32,
    },
    /// Zone cells have no surface a figure can stand on.
    UnstandableZone {
        /// The zone.
        zone: Sym,
    },
    /// No zone is a start zone, so start zones cannot be checked.
    NoStartZones,
    /// A start zone has fewer standable spaces than the army deploying there has figures.
    StartZoneTooSmall {
        /// The zone.
        zone: Sym,
        /// Standable spaces in the zone.
        spaces: u32,
        /// Figures in the army.
        army: u32,
    },
}

/// One problem with a map, with the cells it concerns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// How serious it is.
    pub severity: Severity,
    /// What it is about.
    pub kind: FindingKind,
    /// The cells it concerns, for the editor to highlight; may be empty.
    pub cells: Vec<Hex>,
}

impl Finding {
    /// Creates a finding, with its severity set by its kind.
    pub fn new(kind: FindingKind, cells: Vec<Hex>) -> Self {
        let severity = match kind {
            FindingKind::UnstandableZone { .. } | FindingKind::NoStartZones => Severity::Warning,
            _ => Severity::Error,
        };
        Self {
            severity,
            kind,
            cells,
        }
    }

    /// Turns a board error into a finding kind and the cell it concerns.
    pub(crate) fn from_board(error: &BoardError) -> (FindingKind, Hex) {
        match error.clone() {
            BoardError::UnknownTile { piece, at, name } => {
                (FindingKind::UnknownTile { piece, name }, at)
            }
            BoardError::UnknownFeature { feature, at, name } => {
                (FindingKind::UnknownFeature { feature, name }, at)
            }
            BoardError::Overlap { piece, other, hex } => {
                (FindingKind::Overlap { piece, other }, hex)
            }
            BoardError::Unsupported { piece, hex } => (FindingKind::Unsupported { piece }, hex),
            BoardError::FeatureOffBoard { feature, hex } => {
                (FindingKind::FeatureOffBoard { feature }, hex)
            }
        }
    }
}

impl fmt::Display for FindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTile { piece, name } => {
                write!(f, "piece {piece} names unknown tile `{name}`")
            }
            Self::UnknownFeature { feature, name } => {
                write!(f, "feature {feature} names unknown feature `{name}`")
            }
            Self::FeatureOffBoard { feature } => write!(f, "feature {feature} hangs off the board"),
            Self::Overlap { piece, other } => write!(f, "piece {piece} overlaps piece {other}"),
            Self::Unsupported { piece } => write!(f, "piece {piece} is unsupported"),
            Self::UnknownInventory { name } => write!(f, "unknown inventory `{name}`"),
            Self::InventoryOverrun {
                name,
                used,
                available,
            } => write!(f, "uses {used} `{name}` but the inventory has {available}"),
            Self::UnstandableZone { zone } => write!(f, "zone `{zone}` covers unstandable columns"),
            Self::NoStartZones => write!(f, "no start zones (zones named `start_*`)"),
            Self::StartZoneTooSmall { zone, spaces, army } => write!(
                f,
                "start zone `{zone}` has {spaces} standable spaces for an army of {army}"
            ),
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{severity}: {}", self.kind)?;
        for (i, hex) in self.cells.iter().enumerate() {
            let sep = if i == 0 { "; cells " } else { ", " };
            write!(f, "{sep}({}, {})", hex.q, hex.r)?;
        }
        Ok(())
    }
}
