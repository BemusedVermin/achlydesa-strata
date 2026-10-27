//! Compiling a `MapDef` into a `CompiledBoard`.

use std::collections::BTreeMap;

use strata_kernel::{Hex, Sym};

use super::{BoardError, Column, ColumnSegment, CompiledBoard, FeatureSegment};
use crate::map::MapDef;
use crate::terrain::{FeatureDef, TileDef};

/// Resolves the tile and feature names a map refers to.
pub trait BoardDefs {
    /// Returns the tile called `name`.
    fn tile(&self, name: &Sym) -> Option<&TileDef>;
    /// Returns the feature called `name`.
    fn feature(&self, name: &Sym) -> Option<&FeatureDef>;
}

/// Terrain segments in one column with the index of the piece that placed each.
type Placed = Vec<(usize, ColumnSegment)>;

/// Compiles `map` into columns, checking overlap and, if the map asks for it, support.
///
/// A feature cell stands on the top of the highest terrain segment in its column, and its
/// segment levels are measured from there.
pub fn compile(map: &MapDef, defs: &impl BoardDefs) -> Result<CompiledBoard, BoardError> {
    let mut placed: BTreeMap<Hex, Placed> = BTreeMap::new();
    for (index, piece) in map.pieces.iter().enumerate() {
        let tile = defs
            .tile(&piece.piece)
            .ok_or_else(|| BoardError::UnknownTile {
                piece: index,
                at: piece.at,
                name: piece.piece.clone(),
            })?;
        let z0 = u32::from(piece.level);
        let segment = ColumnSegment {
            z0,
            z1: z0 + u32::from(tile.thickness),
            terrain: tile.terrain,
        };
        if segment.z0 == segment.z1 {
            continue;
        }
        for &hex in tile.shape.rotated(piece.rot).translated(piece.at).cells() {
            let column = placed.entry(hex).or_default();
            let clash = column
                .iter()
                .find(|(_, s)| s.z0 < segment.z1 && segment.z0 < s.z1);
            if let Some(&(other, _)) = clash {
                return Err(BoardError::Overlap {
                    piece: index,
                    other,
                    hex,
                });
            }
            column.push((index, segment));
        }
    }
    if map.constraints.require_support {
        check_support(&placed)?;
    }
    let mut columns: BTreeMap<Hex, Column> = placed
        .into_iter()
        .map(|(hex, segs)| {
            let segments = segs.into_iter().map(|(_, s)| s).collect();
            (
                hex,
                Column {
                    segments,
                    ..Column::default()
                },
            )
        })
        .collect();
    place_features(map, defs, &mut columns)?;
    Ok(CompiledBoard::from_sparse(columns))
}

/// Requires every raised piece cell to rest on the top of another segment.
fn check_support(placed: &BTreeMap<Hex, Placed>) -> Result<(), BoardError> {
    for (&hex, column) in placed {
        for &(piece, segment) in column {
            let supported = segment.z0 == 0 || column.iter().any(|(_, s)| s.z1 == segment.z0);
            if !supported {
                return Err(BoardError::Unsupported { piece, hex });
            }
        }
    }
    Ok(())
}

/// Adds each placed feature's spans to the columns it covers.
fn place_features(
    map: &MapDef,
    defs: &impl BoardDefs,
    columns: &mut BTreeMap<Hex, Column>,
) -> Result<(), BoardError> {
    for (index, placed) in map.features.iter().enumerate() {
        let feature = defs
            .feature(&placed.feature)
            .ok_or_else(|| BoardError::UnknownFeature {
                feature: index,
                at: placed.at,
                name: placed.feature.clone(),
            })?;
        for cell in &feature.cells {
            let hex = cell.at.rotate(placed.rot).add(placed.at);
            let column = columns.get_mut(&hex).ok_or(BoardError::FeatureOffBoard {
                feature: index,
                hex,
            })?;
            let base = column.segments.iter().map(|s| s.z1).max().unwrap_or(0);
            column
                .features
                .extend(cell.segments.iter().map(|s| FeatureSegment {
                    feature: index as u32,
                    z0: base + u32::from(s.from),
                    z1: base + u32::from(s.to),
                    top: s.top,
                    blocks_los: feature.blocks_los,
                }));
        }
    }
    Ok(())
}
