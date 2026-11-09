//! The terrain each hex holds, read straight from a map's pieces.
//!
//! The editor needs this even while the map does not compile (two pieces overlap, a tile is
//! missing), so it never goes through [`CompiledBoard`](strata_model::board::CompiledBoard).

use std::collections::BTreeMap;

use strata_kernel::{DefId, Hex};
use strata_model::MapDef;
use strata_model::board::BoardDefs;

/// A piece's terrain in one column, covering the half-open level interval `[z0, z1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// Bottom level, inclusive.
    pub z0: u32,
    /// Top level, exclusive.
    pub z1: u32,
    /// The terrain of the span.
    pub terrain: DefId,
}

/// The terrain spans of every hex the map's pieces cover.
///
/// Pieces whose tile is unknown or has no thickness cover nothing, as when the board compiles.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Occupancy {
    columns: BTreeMap<Hex, Vec<Span>>,
}

impl Occupancy {
    /// Collects the spans of every piece of `map`.
    pub fn of(map: &MapDef, defs: &impl BoardDefs) -> Self {
        let mut columns: BTreeMap<Hex, Vec<Span>> = BTreeMap::new();
        for piece in &map.pieces {
            let Some(tile) = defs.tile(&piece.piece) else {
                continue;
            };
            let z0 = u32::from(piece.level);
            let span = Span {
                z0,
                z1: z0 + u32::from(tile.thickness),
                terrain: tile.terrain,
            };
            if span.z0 == span.z1 {
                continue;
            }
            for &hex in tile.shape.rotated(piece.rot).translated(piece.at).cells() {
                columns.entry(hex).or_default().push(span);
            }
        }
        Self { columns }
    }

    /// The spans at `hex`, in the order their pieces were placed.
    pub fn spans(&self, hex: Hex) -> &[Span] {
        self.columns.get(&hex).map_or(&[], Vec::as_slice)
    }

    /// The highest span at `hex`, whose top is where a piece snapped onto it rests.
    pub fn top_span(&self, hex: Hex) -> Option<Span> {
        self.spans(hex).iter().copied().max_by_key(|s| s.z1)
    }

    /// The level a piece snapped onto `hex` rests on: the top of its highest span, or 0.
    pub fn top(&self, hex: Hex) -> u32 {
        self.top_span(hex).map_or(0, |s| s.z1)
    }

    /// Whether any span at `hex` overlaps the levels `[z0, z1)`.
    pub fn clashes(&self, hex: Hex, z0: u32, z1: u32) -> bool {
        self.spans(hex).iter().any(|s| s.z0 < z1 && z0 < s.z1)
    }

    /// Whether a piece with bottom level `z0` at `hex` rests on level 0 or on a span's top.
    pub fn supported(&self, hex: Hex, z0: u32) -> bool {
        z0 == 0 || self.spans(hex).iter().any(|s| s.z1 == z0)
    }

    /// Whether any piece covers `hex`.
    pub fn covers(&self, hex: Hex) -> bool {
        self.columns.contains_key(&hex)
    }

    /// The corners `(min, max)` of the axial bounding box of every covered hex, or `None` for a
    /// map with no terrain.
    pub fn bounds(&self) -> Option<(Hex, Hex)> {
        let mut hexes = self.columns.keys();
        let first = *hexes.next()?;
        Some(hexes.fold((first, first), |(min, max), hex| {
            (
                Hex::new(min.q.min(hex.q), min.r.min(hex.r)),
                Hex::new(max.q.max(hex.q), max.r.max(hex.r)),
            )
        }))
    }
}
