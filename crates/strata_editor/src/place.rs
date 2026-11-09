//! Planning a placement: which level a piece goes at, whether it fits, and the edit that makes
//! it (design §14.2). Nothing here changes the map; the caller applies the planned edit.

use strata_kernel::{Hex, Rotation, Space, Sym};
use strata_model::{MapDef, PlacedFeature, PlacedPiece};

use crate::defs::EditorDefs;
use crate::document::MapDocument;
use crate::edit::Edit;
use crate::occupancy::Occupancy;
use crate::palette::{PaletteItem, Stock};

/// How the placement ghost picks its level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LevelMode {
    /// Rest on the top surface under the cursor: the top of the highest terrain in the column of
    /// the piece's anchor hex, or level 0 on open ground (design §14.2).
    ///
    /// Only the anchor's column counts. A multi-hex piece whose other hexes are higher overlaps
    /// there, and one whose other hexes are lower floats there; both are reported. Under a
    /// bridge or an arch the highest terrain is the deck, so building beneath one takes a locked
    /// level.
    #[default]
    Snap,
    /// Put the piece's bottom at this level, whatever is under it.
    Locked(u8),
}

impl LevelMode {
    /// The bottom level for a piece anchored at `at`, or `None` when snapping would put it above
    /// the highest level a map can hold.
    pub fn level(self, occupancy: &Occupancy, at: Hex) -> Option<u8> {
        match self {
            Self::Locked(level) => Some(level),
            Self::Snap => u8::try_from(occupancy.top(at)).ok(),
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
    /// A fill on open ground reaches the edge of the map's terrain, so it has no bounds.
    NotEnclosed,
    /// The fill's region has more than [`FILL_LIMIT`](crate::FILL_LIMIT) hexes.
    TooLarge,
}

impl Problem {
    /// Whether the problem refuses the placement. Only running out of stock is a warning: the
    /// map still loads, but validation reports the overrun as an error, so `strata map check`
    /// fails until the inventory or the map is changed.
    ///
    /// Whether freeform painting counts against the inventory at all is an open question:
    /// design §11 says it has no inventory limit, while validation counts every piece.
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

    /// A preview that places nothing, outlines `at` as refused and says why.
    pub(crate) fn refused(problem: Problem, at: Space) -> Self {
        Self {
            marks: vec![Mark {
                space: at,
                ok: false,
            }],
            problems: vec![problem],
            edits: Vec::new(),
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

/// What a plan reads: the map, its definitions and the facts the document keeps about them.
pub(crate) struct Site<'a, D> {
    pub(crate) map: &'a MapDef,
    pub(crate) defs: &'a D,
    pub(crate) occupancy: &'a Occupancy,
    pub(crate) stock: Option<&'a Stock>,
}

impl<'a, D: EditorDefs> Site<'a, D> {
    /// Reads `document` as it is now.
    pub(crate) fn new(document: &'a MapDocument<D>) -> Self {
        Self {
            map: document.map(),
            defs: document.defs(),
            occupancy: document.occupancy(),
            stock: document.stock(),
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
        let name = match self.one_hex_tile(item) {
            Ok(name) => name,
            Err(problem) => {
                preview.report(problem, None);
                return preview;
            }
        };
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

    /// The name of `item` if it is a one-hex tile, the only thing line and fill paint.
    pub(crate) fn one_hex_tile<'i>(&self, item: &'i PaletteItem) -> Result<&'i Sym, Problem> {
        let tile = match item {
            PaletteItem::Tile(name) => self.defs.tile(name).map(|t| (name, t)),
            PaletteItem::Feature(_) => None,
        };
        match tile {
            Some((name, tile)) if tile.shape.cells() == [Hex::ORIGIN] => Ok(name),
            Some(_) => Err(Problem::NotOneHex),
            None if item.id(self.defs).is_some() => Err(Problem::NotOneHex),
            None => Err(Problem::UnknownItem),
        }
    }

    /// Checks a tile at `at`, adding its marks and problems, and returns its edit.
    fn place_tile(
        &self,
        name: &Sym,
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
        let Some(level) = mode.level(self.occupancy, at) else {
            preview.report(Problem::TooHigh(Vec::new()), Some(at));
            for &hex in &cells {
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
        name: &Sym,
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
        let Some(stock) = self.stock else { return };
        let remaining = stock.remaining(item, self.defs) - count;
        if count > 0 && remaining < 0 {
            preview.report(Problem::OutOfStock { remaining }, None);
        }
    }
}
