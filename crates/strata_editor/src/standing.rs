//! Where a figure can stand on a compiled board.

use strata_kernel::{DefId, Hex, Space};
use strata_model::board::CompiledBoard;

use crate::defs::EditorDefs;

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
