//! Walking distances for a reference one-space figure, found with `core:movement`'s search
//! (design §7.4, §14.1).

use std::collections::BTreeMap;

use strata_kernel::{DefId, Hex, Space};
use strata_model::FigureId;
use strata_model::board::CompiledBoard;
use strata_modules::movement::{Headroom, Limits, reachable};
use strata_rules::{LogicError, MoveWorld, TerrainRules};

use crate::defs::EditorDefs;
use crate::standing::standable;

/// The reference figure's limits: `core:movement`'s default climb of one level, headroom for
/// its height, and no fall limit, because the analysis measures a walk over many turns.
const LIMITS: Limits = Limits {
    headroom: Headroom::FigureHeight,
    max_climb: 1,
    max_fall: u32::MAX,
};

/// The board as the reference figure sees it: alone on the board, one level tall, with an
/// unlimited allowance, every step costing one, and no terrain ending its move.
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

/// The fewest steps from any of `starts` to each column the reference figure can stand in.
///
/// Each start column counts as zero steps. Columns the figure cannot reach are absent.
pub(crate) fn distances(
    board: &CompiledBoard,
    defs: &impl EditorDefs,
    starts: &[Space],
) -> BTreeMap<Hex, u32> {
    let mut best = BTreeMap::new();
    let mut keep = |hex: Hex, steps: u32| {
        let entry = best.entry(hex).or_insert(steps);
        *entry = (*entry).min(steps);
    };
    for &start in starts {
        keep(start.hex(), 0);
        let world = ReferenceWorld { board, defs, start };
        // The reference world answers every query without failing, so there is no error to see.
        let found = reachable(&world, FigureId(0), LIMITS).unwrap_or_default();
        for reach in found {
            keep(
                reach.space.hex(),
                u32::try_from(reach.cost).unwrap_or(u32::MAX),
            );
        }
    }
    best
}
