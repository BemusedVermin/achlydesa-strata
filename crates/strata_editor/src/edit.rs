//! Edit commands on a `MapDef`, each of which yields the command that undoes it (design §14.1).

use std::collections::BTreeSet;

use strata_kernel::{Hex, Rotation, Sym, Value};
use strata_model::{MapConstraints, MapDef, PlacedFeature, PlacedPiece, Text};
use thiserror::Error;

/// Why an edit could not be applied. The map is unchanged when this is returned.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EditError {
    /// The piece index is out of range.
    #[error("no piece at index {0}")]
    NoSuchPiece(usize),
    /// The feature index is out of range.
    #[error("no feature at index {0}")]
    NoSuchFeature(usize),
    /// The named zone does not exist.
    #[error("no zone named `{0}`")]
    NoSuchZone(Sym),
    /// The batch has no edits in it, so it would be an undo step that does nothing.
    #[error("the batch has no edits")]
    EmptyBatch,
}

/// A change to the map's descriptive fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Metadata {
    /// Sets the display name.
    Name(Text),
    /// Sets the author names.
    Authors(Vec<String>),
    /// Sets the rendering orientation.
    Orientation(Sym),
    /// Sets the build rules.
    Constraints(MapConstraints),
    /// Sets the free-form tags.
    Tags(BTreeSet<Sym>),
    /// Sets (`Some`) or removes (`None`) one entry of the free-form data.
    Entry {
        /// The entry's key.
        key: Sym,
        /// The new value, or `None` to remove the entry.
        value: Option<Value>,
    },
}

impl Metadata {
    /// Writes this change to `map` and returns the change that restores the previous value.
    fn swap(self, map: &mut MapDef) -> Metadata {
        match self {
            Self::Name(v) => Self::Name(std::mem::replace(&mut map.name, v)),
            Self::Authors(v) => Self::Authors(std::mem::replace(&mut map.authors, v)),
            Self::Orientation(v) => Self::Orientation(std::mem::replace(&mut map.orientation, v)),
            Self::Constraints(v) => Self::Constraints(std::mem::replace(&mut map.constraints, v)),
            Self::Tags(v) => Self::Tags(std::mem::replace(&mut map.tags, v)),
            Self::Entry { key, value } => {
                let old = match value {
                    Some(v) => map.meta.insert(key.clone(), v),
                    None => map.meta.remove(&key),
                };
                Self::Entry { key, value: old }
            }
        }
    }
}

/// One undoable change to a map.
///
/// `Insert*`, `SetZone` and the `Remove*` commands exist mainly as the inverses of the others, but
/// they are valid edits in their own right.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    /// Appends a piece.
    PlacePiece(PlacedPiece),
    /// Inserts a piece at `index`, shifting later pieces up.
    InsertPiece {
        /// Position in `MapDef::pieces`.
        index: usize,
        /// The piece to insert.
        piece: PlacedPiece,
    },
    /// Removes the piece at `index`.
    RemovePiece {
        /// Position in `MapDef::pieces`.
        index: usize,
    },
    /// Moves a piece to a new anchor and bottom level.
    MovePiece {
        /// Position in `MapDef::pieces`.
        index: usize,
        /// The new anchor.
        at: Hex,
        /// The new bottom level.
        level: u8,
    },
    /// Sets the rotation of a piece.
    RotatePiece {
        /// Position in `MapDef::pieces`.
        index: usize,
        /// The new rotation.
        rot: Rotation,
    },
    /// Appends a feature.
    PlaceFeature(PlacedFeature),
    /// Inserts a feature at `index`, shifting later features up.
    InsertFeature {
        /// Position in `MapDef::features`.
        index: usize,
        /// The feature to insert.
        feature: PlacedFeature,
    },
    /// Removes the feature at `index`.
    RemoveFeature {
        /// Position in `MapDef::features`.
        index: usize,
    },
    /// Adds cells to a zone, creating it if needed. Cells already in the zone are skipped.
    PaintZone {
        /// The zone's name.
        zone: Sym,
        /// The cells to add.
        cells: Vec<Hex>,
    },
    /// Removes cells from a zone, deleting the zone if it ends up empty.
    EraseZone {
        /// The zone's name.
        zone: Sym,
        /// The cells to remove.
        cells: Vec<Hex>,
    },
    /// Replaces a zone's cells wholesale, or deletes the zone when `cells` is `None`.
    SetZone {
        /// The zone's name.
        zone: Sym,
        /// The new cells.
        cells: Option<Vec<Hex>>,
    },
    /// Sets a relic spot, or clears it when `at` is `None`.
    SetRelicSpot {
        /// The spot's name.
        name: Sym,
        /// The new cell.
        at: Option<Hex>,
    },
    /// Changes a descriptive field.
    SetMetadata(Metadata),
    /// Applies several edits in order as one step, so one undo reverts them all. A brush stroke
    /// that places many one-hex tiles is one batch.
    Batch(Vec<Edit>),
}

impl Edit {
    /// Returns whether this edit can change the compiled board.
    ///
    /// Constraints count because `require_support` changes what compiles.
    pub fn affects_board(&self) -> bool {
        match self {
            Self::PaintZone { .. }
            | Self::EraseZone { .. }
            | Self::SetZone { .. }
            | Self::SetRelicSpot { .. } => false,
            Self::SetMetadata(change) => matches!(change, Metadata::Constraints(_)),
            Self::Batch(edits) => edits.iter().any(Edit::affects_board),
            _ => true,
        }
    }

    /// Returns whether this is a batch with no edits in it, nested batches included.
    pub fn is_empty_batch(&self) -> bool {
        match self {
            Self::Batch(edits) => edits.iter().all(Edit::is_empty_batch),
            _ => false,
        }
    }

    /// Applies the edit to `map` and returns the edit that undoes it.
    ///
    /// Applying the returned edit restores `map` exactly, including the order of pieces and
    /// features. On error the map is unchanged.
    pub fn apply(self, map: &mut MapDef) -> Result<Edit, EditError> {
        Ok(match self {
            Self::PlacePiece(piece) => {
                map.pieces.push(piece);
                Self::RemovePiece {
                    index: map.pieces.len() - 1,
                }
            }
            Self::InsertPiece { index, piece } => {
                if index > map.pieces.len() {
                    return Err(EditError::NoSuchPiece(index));
                }
                map.pieces.insert(index, piece);
                Self::RemovePiece { index }
            }
            Self::RemovePiece { index } => {
                if index >= map.pieces.len() {
                    return Err(EditError::NoSuchPiece(index));
                }
                let piece = map.pieces.remove(index);
                Self::InsertPiece { index, piece }
            }
            Self::MovePiece { index, at, level } => {
                let piece = map
                    .pieces
                    .get_mut(index)
                    .ok_or(EditError::NoSuchPiece(index))?;
                let old = (piece.at, piece.level);
                (piece.at, piece.level) = (at, level);
                Self::MovePiece {
                    index,
                    at: old.0,
                    level: old.1,
                }
            }
            Self::RotatePiece { index, rot } => {
                let piece = map
                    .pieces
                    .get_mut(index)
                    .ok_or(EditError::NoSuchPiece(index))?;
                let old = std::mem::replace(&mut piece.rot, rot);
                Self::RotatePiece { index, rot: old }
            }
            Self::PlaceFeature(feature) => {
                map.features.push(feature);
                Self::RemoveFeature {
                    index: map.features.len() - 1,
                }
            }
            Self::InsertFeature { index, feature } => {
                if index > map.features.len() {
                    return Err(EditError::NoSuchFeature(index));
                }
                map.features.insert(index, feature);
                Self::RemoveFeature { index }
            }
            Self::RemoveFeature { index } => {
                if index >= map.features.len() {
                    return Err(EditError::NoSuchFeature(index));
                }
                let feature = map.features.remove(index);
                Self::InsertFeature { index, feature }
            }
            Self::PaintZone { zone, cells } => {
                let old = map.zones.get(&zone).cloned();
                let column = map.zones.entry(zone.clone()).or_default();
                for hex in cells {
                    if !column.contains(&hex) {
                        column.push(hex);
                    }
                }
                Self::SetZone { zone, cells: old }
            }
            Self::EraseZone { zone, cells } => {
                let column = map
                    .zones
                    .get_mut(&zone)
                    .ok_or_else(|| EditError::NoSuchZone(zone.clone()))?;
                let old = column.clone();
                column.retain(|hex| !cells.contains(hex));
                if column.is_empty() {
                    map.zones.remove(&zone);
                }
                Self::SetZone {
                    zone,
                    cells: Some(old),
                }
            }
            Self::SetZone { zone, cells } => {
                let old = match cells {
                    Some(cells) => map.zones.insert(zone.clone(), cells),
                    None => map.zones.remove(&zone),
                };
                Self::SetZone { zone, cells: old }
            }
            Self::SetRelicSpot { name, at } => {
                let old = match at {
                    Some(hex) => map.relic_spots.insert(name.clone(), hex),
                    None => map.relic_spots.remove(&name),
                };
                Self::SetRelicSpot { name, at: old }
            }
            Self::SetMetadata(change) => Self::SetMetadata(change.swap(map)),
            Self::Batch(edits) => Self::Batch(apply_batch(edits, map)?),
        })
    }
}

/// Applies `edits` in order and returns their inverses in reverse order. If one fails, the edits
/// already applied are undone, so the map is unchanged.
fn apply_batch(edits: Vec<Edit>, map: &mut MapDef) -> Result<Vec<Edit>, EditError> {
    let mut inverses = Vec::with_capacity(edits.len());
    for edit in edits {
        match edit.apply(map) {
            Ok(inverse) => inverses.push(inverse),
            Err(error) => {
                for inverse in inverses.into_iter().rev() {
                    // Each inverse restores the map its edit saw, so it cannot fail.
                    let _ = inverse.apply(map);
                }
                return Err(error);
            }
        }
    }
    inverses.reverse();
    Ok(inverses)
}
