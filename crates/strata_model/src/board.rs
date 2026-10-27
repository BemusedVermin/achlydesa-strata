//! The compiled board: columns of terrain and feature segments (design §7.2).

use std::collections::BTreeMap;

use strata_kernel::{Blocker, BlockerFootprint, DefId, Hex, SUB_LEVELS};

pub use self::compile::{BoardDefs, compile};
pub use self::error::BoardError;

mod compile;
mod error;
#[cfg(test)]
mod tests;

/// A solid terrain span covering the half-open level interval `[z0, z1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnSegment {
    /// Bottom level, inclusive.
    pub z0: u32,
    /// Top level, exclusive.
    pub z1: u32,
    /// The terrain of the span.
    pub terrain: DefId,
}

/// A level a figure can stand on, at the top of a terrain segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surface {
    /// The level of the surface.
    pub z: u32,
    /// Free levels above it before the next terrain segment, or `None` if nothing is above.
    pub headroom: Option<u32>,
    /// The terrain underfoot.
    pub terrain: DefId,
}

/// A span of a placed feature, in absolute levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureSegment {
    /// Index into the map's placed features.
    pub feature: u32,
    /// Bottom level, inclusive.
    pub z0: u32,
    /// Top level, exclusive.
    pub z1: u32,
    /// Whether a figure can stand on top of the span.
    pub top: bool,
    /// Whether the span blocks line of sight.
    pub blocks_los: bool,
}

/// Everything standing on one hex.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Column {
    /// Terrain segments, ordered bottom to top.
    pub segments: Vec<ColumnSegment>,
    /// Surfaces derived from the terrain segments, ordered bottom to top.
    pub surfaces: Vec<Surface>,
    /// Feature spans in this column.
    pub features: Vec<FeatureSegment>,
    /// Line-of-sight blockers: all terrain plus the features that block sight.
    pub blockers: Vec<Blocker>,
}

impl Column {
    /// Derives surfaces and blockers from the segments and feature spans.
    fn finish(&mut self) {
        self.segments.sort_by_key(|s| s.z0);
        self.surfaces = self
            .segments
            .iter()
            .enumerate()
            .filter_map(|(i, seg)| {
                let headroom = self.segments.get(i + 1).map(|next| next.z0 - seg.z1);
                (headroom != Some(0)).then_some(Surface {
                    z: seg.z1,
                    headroom,
                    terrain: seg.terrain,
                })
            })
            .collect();
        let terrain = self.segments.iter().map(|s| (s.z0, s.z1));
        let features = self
            .features
            .iter()
            .filter(|f| f.blocks_los)
            .map(|f| (f.z0, f.z1));
        self.blockers = terrain
            .chain(features)
            .map(|(z0, z1)| Blocker {
                footprint: BlockerFootprint::FullHex,
                z0: i64::from(z0) * SUB_LEVELS,
                z1: i64::from(z1) * SUB_LEVELS,
            })
            .collect();
    }
}

/// A dense grid of columns over the bounding box of the map's pieces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledBoard {
    min: Hex,
    width: i32,
    height: i32,
    columns: Vec<Column>,
}

impl CompiledBoard {
    /// Builds the grid from sparse columns; hexes inside the bounding box with no piece stay empty.
    fn from_sparse(mut sparse: BTreeMap<Hex, Column>) -> Self {
        let Some(first) = sparse.keys().next().copied() else {
            return Self {
                min: Hex::ORIGIN,
                width: 0,
                height: 0,
                columns: Vec::new(),
            };
        };
        let (mut min, mut max) = (first, first);
        for hex in sparse.keys() {
            min = Hex::new(min.q.min(hex.q), min.r.min(hex.r));
            max = Hex::new(max.q.max(hex.q), max.r.max(hex.r));
        }
        let (width, height) = (max.q - min.q + 1, max.r - min.r + 1);
        let columns = (0..height)
            .flat_map(|r| (0..width).map(move |q| Hex::new(min.q + q, min.r + r)))
            .map(|hex| {
                let mut column = sparse.remove(&hex).unwrap_or_default();
                column.finish();
                column
            })
            .collect();
        Self {
            min,
            width,
            height,
            columns,
        }
    }

    /// Returns the column at `hex`, or `None` outside the bounding box.
    pub fn column(&self, hex: Hex) -> Option<&Column> {
        let (dq, dr) = (hex.q - self.min.q, hex.r - self.min.r);
        if !(0..self.width).contains(&dq) || !(0..self.height).contains(&dr) {
            return None;
        }
        self.columns
            .get(usize::try_from(dr * self.width + dq).ok()?)
    }

    /// Returns the surfaces at `hex`, bottom to top.
    pub fn surfaces(&self, hex: Hex) -> &[Surface] {
        self.column(hex).map_or(&[], |c| &c.surfaces)
    }

    /// Returns the highest surface at `hex`.
    pub fn top_surface(&self, hex: Hex) -> Option<Surface> {
        self.surfaces(hex).last().copied()
    }

    /// Returns whether level `z` at `hex` is inside terrain or a feature span.
    pub fn is_solid(&self, hex: Hex, z: u32) -> bool {
        self.column(hex).is_some_and(|c| {
            c.segments.iter().any(|s| (s.z0..s.z1).contains(&z))
                || c.features.iter().any(|f| (f.z0..f.z1).contains(&z))
        })
    }

    /// Returns the line-of-sight blockers at `hex`, for the kernel's occlusion tests.
    pub fn blockers(&self, hex: Hex) -> &[Blocker] {
        self.column(hex).map_or(&[], |c| &c.blockers)
    }
}
