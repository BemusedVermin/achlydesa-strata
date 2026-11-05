//! Walking distances for a reference one-space figure, using `core:movement`'s search (§7.4).

use std::collections::BTreeMap;

use strata_kernel::{DefId, Hex, Space};
use strata_model::FigureId;
use strata_model::board::CompiledBoard;
use strata_modules::movement::{Headroom, Limits, reachable};
use strata_rules::{LogicError, MoveWorld, TerrainRules};

use crate::defs::EditorDefs;

/// The reference figure's movement settings: `core:movement`'s default climb, a height of one
/// level, and no fall limit, because the analysis measures a walk over many turns, not one move.
const LIMITS: Limits = Limits {
    headroom: Headroom::FigureHeight,
    max_climb: 1,
    max_fall: u32::MAX,
};

/// The board as the reference figure sees it: alone, every step costing one, no allowance.
struct ReferenceWorld<'a, D> {
    board: &'a CompiledBoard,
    defs: &'a D,
    start: Space,
}

impl<D: EditorDefs> MoveWorld for ReferenceWorld<'_, D> {
    fn board(&self) -> &CompiledBoard {
        self.board
    }
    fn location(&self, _: FigureId) -> Option<Space> {
        Some(self.start)
    }
    fn occupant(&self, _: Space) -> Option<FigureId> {
        None
    }
    fn height(&self, _: FigureId) -> u32 {
        1
    }
    fn allowance(&self, _: FigureId) -> Result<i64, LogicError> {
        Ok(i64::MAX)
    }
    fn terrain(&self, terrain: DefId) -> TerrainRules {
        TerrainRules {
            passable: standable(self.defs, terrain),
            stops_here: false,
        }
    }
    fn move_cost(&self, _: FigureId, _: DefId, _: i32) -> Result<i64, LogicError> {
        Ok(1)
    }
    fn can_climb(&self, _: FigureId, _: DefId, _: i32) -> Result<bool, LogicError> {
        Ok(true)
    }
    fn can_enter(&self, _: FigureId, _: Space) -> Result<bool, LogicError> {
        Ok(true)
    }
}

/// Returns whether a figure can stand on `terrain`. Unknown terrain cannot be stood on.
pub(crate) fn standable(defs: &impl EditorDefs, terrain: DefId) -> bool {
    defs.terrain(terrain).is_some_and(|t| t.standable)
}

/// The spaces in `cells` a figure can stand on, bottom to top within each column.
pub(crate) fn standable_spaces(
    board: &CompiledBoard,
    defs: &impl EditorDefs,
    cells: &[Hex],
) -> Vec<Space> {
    let surfaces = cells.iter().flat_map(|&hex| {
        let on = board.surfaces(hex).iter();
        on.filter(|s| standable(defs, s.terrain))
            .map(move |s| Space::at(hex, s.z))
    });
    surfaces.collect()
}

/// The fewest steps from any of `starts` to each column the reference figure can reach.
pub(crate) fn distances(
    board: &CompiledBoard,
    defs: &impl EditorDefs,
    starts: &[Space],
) -> BTreeMap<Hex, i64> {
    let mut best = BTreeMap::new();
    let mut keep = |hex: Hex, cost: i64| {
        let entry = best.entry(hex).or_insert(cost);
        *entry = (*entry).min(cost);
    };
    for &start in starts {
        keep(start.hex(), 0);
        let world = ReferenceWorld { board, defs, start };
        // The reference world answers every query without failing.
        let found = reachable(&world, FigureId(0), LIMITS).unwrap_or_default();
        for reach in found {
            keep(reach.space.hex(), reach.cost);
        }
    }
    best
}
