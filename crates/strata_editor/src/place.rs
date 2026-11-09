//! Planning a placement: which level a piece goes at, whether it fits, and the edit that makes
//! it (design §14.2). Nothing here changes the map; the caller applies the planned edit.

use strata_kernel::{Hex, Rotation, Space};
use strata_model::{MapDef, PlacedFeature, PlacedPiece};

use crate::defs::EditorDefs;
use crate::edit::Edit;
use crate::occupancy::Occupancy;
use crate::palette::{PaletteItem, Stock};

/// How the placement ghost picks its level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LevelMode {
    /// Rest on the highest terrain under any of the piece's hexes, or on level 0.
    #[default]
    Snap,
    /// Put the piece's bottom at this level, whatever is under it.
    Locked(u8),
}

impl LevelMode {
    /// The bottom level for a piece covering `cells`, or `None` when snapping would put it above
    /// the highest level a map can hold.
    pub fn level(self, occupancy: &Occupancy, cells: &[Hex]) -> Option<u8> {
        match self {
            Self::Locked(level) => Some(level),
            Self::Snap => {
                let top = cells.iter().map(|&h| occupancy.top(h)).max().unwrap_or(0);
                u8::try_from(top).ok()
            }
        }
    }
}

/// Why a planned placement, or part of a stroke, is refused or worth a warning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The item's name does not resolve to a definition.
    UnknownItem,
    /// The line and fill brushes paint one-hex tiles only.
    NotOneHex,
    /// These hexes already hold terrain at the piece's levels.
    Overlap(Vec<Hex>),
    /// The map requires support and these hexes would float.
    Unsupported(Vec<Hex>),
    /// These hexes of a feature have no terrain to stand on.
    OffBoard(Vec<Hex>),
    /// Snapping would put the piece above the highest level a map can hold, at these hexes.
    TooHigh(Vec<Hex>),
    /// The inventory has none of the item left; `remaining` is what it has after this placement.
    OutOfStock {
        /// The count left once the placement is made; negative means over the inventory.
        remaining: i64,
    },
}

impl Problem {
    /// Whether the problem refuses the placement. Only running out of stock is a warning:
    /// validation reports it, and the map still loads.
    pub fn blocks(&self) -> bool {
        !matches!(self, Self::OutOfStock { .. })
    }

    /// Adds `hex` to this problem's hexes, if it lists any.
    fn add(&mut self, hex: Hex) {
        if let Self::Overlap(hexes)
        | Self::Unsupported(hexes)
        | Self::OffBoard(hexes)
        | Self::TooHigh(hexes) = self
            && !hexes.contains(&hex)
        {
            hexes.push(hex);
        }
    }
}

/// One outlined hex of the ghost: the top face the placed item would have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    /// The hex and the level of the face.
    pub space: Space,
    /// Whether this hex would be placed.
    pub ok: bool,
}

/// What releasing the tool at the cursor would do.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preview {
    /// The hexes to outline.
    pub marks: Vec<Mark>,
    /// Every problem found, each once, with the hexes it concerns.
    pub problems: Vec<Problem>,
    /// The placements to make, in order.
    pub edits: Vec<Edit>,
}

impl Preview {
    /// The single edit that makes every planned placement, or `None` if nothing would be placed.
    /// Several placements become one [`Edit::Batch`], so one undo reverts the stroke.
    pub fn edit(&self) -> Option<Edit> {
        match self.edits.as_slice() {
            [] => None,
            [one] => Some(one.clone()),
            many => Some(Edit::Batch(many.to_vec())),
        }
    }

    /// Records `problem` at `hex`, merging it into an earlier problem of the same kind.
    fn report(&mut self, mut problem: Problem, hex: Option<Hex>) {
        let same =
            |p: &&mut Problem| std::mem::discriminant(*p) == std::mem::discriminant(&problem);
        if let Some(found) = self.problems.iter_mut().find(same) {
            if let Some(hex) = hex {
                found.add(hex);
            }
            if let (Problem::OutOfStock { remaining }, Problem::OutOfStock { remaining: now }) =
                (found, &problem)
            {
                *remaining = (*remaining).min(*now);
            }
            return;
        }
        if let Some(hex) = hex {
            problem.add(hex);
        }
        self.problems.push(problem);
    }
}

/// What a plan reads: the map, its definitions and the facts derived from them.
pub(crate) struct Site<'a, D> {
    pub(crate) map: &'a MapDef,
    pub(crate) defs: &'a D,
    pub(crate) occupancy: Occupancy,
    pub(crate) stock: Option<Stock>,
}

impl<'a, D: EditorDefs> Site<'a, D> {
    /// Reads `map` against `defs`.
    pub(crate) fn new(map: &'a MapDef, defs: &'a D) -> Self {
        Self {
            map,
            defs,
            occupancy: Occupancy::of(map, defs),
            stock: Stock::of(map, defs),
        }
    }

    /// Plans placing `item` once with its anchor at `at`, all or nothing.
    pub(crate) fn place(
        &self,
        item: &PaletteItem,
        at: Hex,
        rot: Rotation,
        mode: LevelMode,
    ) -> Preview {
        let mut preview = Preview::default();
        let planned = match item {
            PaletteItem::Tile(name) => self.place_tile(name, at, rot, mode, &mut preview),
            PaletteItem::Feature(name) => self.place_feature(name, at, rot, &mut preview),
        };
        let Some(edit) = planned else {
            return preview;
        };
        self.count_stock(item, 1, &mut preview);
        let ok = !preview.problems.iter().any(Problem::blocks);
        for mark in &mut preview.marks {
            mark.ok = ok;
        }
        if ok {
            preview.edits.push(edit);
        }
        preview
    }

    /// Plans a stroke that puts the one-hex tile `item` on each of `cells`, skipping the hexes
    /// where it does not fit. Each hex snaps on its own.
    pub(crate) fn stroke(&self, item: &PaletteItem, cells: &[Hex], mode: LevelMode) -> Preview {
        let mut preview = Preview::default();
        let tile = match item {
            PaletteItem::Tile(name) => self.defs.tile(name).map(|t| (name, t)),
            PaletteItem::Feature(_) => None,
        };
        let Some((name, tile)) = tile else {
            let problem = match item.id(self.defs) {
                Some(_) => Problem::NotOneHex,
                None => Problem::UnknownItem,
            };
            preview.report(problem, None);
            return preview;
        };
        if tile.shape.cells() != [Hex::ORIGIN] {
            preview.report(Problem::NotOneHex, None);
            return preview;
        }
        for &hex in cells {
            let mut one = Preview::default();
            let edit = self.place_tile(name, hex, Rotation::default(), mode, &mut one);
            let edit = edit.filter(|_| !one.problems.iter().any(Problem::blocks));
            let ok = edit.is_some();
            for problem in one.problems {
                preview.report(problem, Some(hex));
            }
            preview
                .marks
                .extend(one.marks.into_iter().map(|m| Mark { ok, ..m }));
            preview.edits.extend(edit);
        }
        let placed = preview.edits.len() as i64;
        self.count_stock(item, placed, &mut preview);
        preview
    }

    /// Checks a tile at `at`, adding its marks and problems, and returns its edit.
    fn place_tile(
        &self,
        name: &strata_kernel::Sym,
        at: Hex,
        rot: Rotation,
        mode: LevelMode,
        preview: &mut Preview,
    ) -> Option<Edit> {
        let Some(tile) = self.defs.tile(name) else {
            preview.report(Problem::UnknownItem, None);
            return None;
        };
        let cells = tile.shape.rotated(rot).translated(at).cells().to_vec();
        let Some(level) = mode.level(&self.occupancy, &cells) else {
            for &hex in &cells {
                preview.report(Problem::TooHigh(Vec::new()), Some(hex));
                let top = self.occupancy.top(hex);
                preview.marks.push(Mark {
                    space: Space::at(hex, top),
                    ok: false,
                });
            }
            return None;
        };
        let (z0, z1) = (
            u32::from(level),
            u32::from(level) + u32::from(tile.thickness),
        );
        for &hex in &cells {
            if self.occupancy.clashes(hex, z0, z1) {
                preview.report(Problem::Overlap(Vec::new()), Some(hex));
            }
            if self.map.constraints.require_support && !self.occupancy.supported(hex, z0) {
                preview.report(Problem::Unsupported(Vec::new()), Some(hex));
            }
            preview.marks.push(Mark {
                space: Space::at(hex, z1),
                ok: true,
            });
        }
        Some(Edit::PlacePiece(PlacedPiece {
            piece: name.clone(),
            at,
            rot,
            level,
        }))
    }

    /// Checks a feature at `at`, adding its marks and problems, and returns its edit.
    fn place_feature(
        &self,
        name: &strata_kernel::Sym,
        at: Hex,
        rot: Rotation,
        preview: &mut Preview,
    ) -> Option<Edit> {
        let Some(feature) = self.defs.feature(name) else {
            preview.report(Problem::UnknownItem, None);
            return None;
        };
        for cell in &feature.cells {
            let hex = cell.at.rotate(rot).add(at);
            if !self.occupancy.covers(hex) {
                preview.report(Problem::OffBoard(Vec::new()), Some(hex));
            }
            let height = cell
                .segments
                .iter()
                .map(|s| u32::from(s.to))
                .max()
                .unwrap_or(0);
            let z = self.occupancy.top(hex) + height;
            preview.marks.push(Mark {
                space: Space::at(hex, z),
                ok: true,
            });
        }
        Some(Edit::PlaceFeature(PlacedFeature {
            feature: name.clone(),
            at,
            rot,
        }))
    }

    /// Warns when placing `count` of `item` leaves the inventory short.
    fn count_stock(&self, item: &PaletteItem, count: i64, preview: &mut Preview) {
        let Some(stock) = &self.stock else { return };
        let remaining = stock.remaining(item, self.defs) - count;
        if count > 0 && remaining < 0 {
            preview.report(Problem::OutOfStock { remaining }, None);
        }
    }
}
