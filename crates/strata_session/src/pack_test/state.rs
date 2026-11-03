//! The starting state of a test: figures placed directly, with no deployment.

use std::sync::Arc;

use strata_content::ContentRegistry;
use strata_kernel::DefId;
use strata_model::{
    FigureDefRef, FigureId, FigureState, FigureStatus, GameState, MatchSetup, PlayerId,
    PlayerState, TestDef, UnitDef, UnitId, UnitState,
};

use super::host::find;

/// Builds the state a test starts from, for the ruleset `ruleset`.
pub(super) fn build(
    registry: &ContentRegistry,
    pack: &str,
    ruleset: DefId,
    test: &TestDef,
) -> Result<GameState, String> {
    let mut state = GameState {
        setup: Arc::new(MatchSetup {
            ruleset,
            ..MatchSetup::default()
        }),
        ..GameState::default()
    };
    let players = test.place.iter().map(|p| p.player.index() + 1).max();
    state.players = (0..players.unwrap_or(0).max(2))
        .map(|team| PlayerState {
            team: team as u8,
            hand: Vec::new(),
        })
        .collect();
    for placement in &test.place {
        let name = placement.unit.as_str();
        let (def_id, def) = find::<UnitDef>(registry, pack, name)
            .ok_or_else(|| format!("place: unknown unit `{name}`"))?;
        let wanted: usize = def.figures.iter().map(|f| usize::from(f.count)).sum();
        if placement.at.len() != wanted {
            let got = placement.at.len();
            return Err(format!(
                "place: `{name}` has {wanted} figures, but `at` has {got}"
            ));
        }
        let unit = UnitId(state.units.len() as u32);
        let copy = state
            .units
            .iter()
            .filter(|u| u.def == def_id && u.owner == placement.player)
            .count();
        let mut unit_state = UnitState {
            def: def_id,
            owner: placement.player,
            copy: copy as u8,
            ..UnitState::default()
        };
        let entries = def.figures.iter();
        let kinds = entries.flat_map(|f| std::iter::repeat_n(&f.id, usize::from(f.count)));
        for (figure_name, space) in kinds.zip(&placement.at) {
            let id = FigureId(state.figures.len() as u32);
            if state.board_changes.occupancy.insert(*space, id).is_some() {
                return Err(format!("place: two figures share space {space:?}"));
            }
            unit_state.figures.push(id);
            state.figures.push(FigureState {
                unit,
                def: FigureDefRef {
                    unit: def_id,
                    figure: figure_name.clone(),
                },
                location: Some(*space),
                status: FigureStatus::OnBoard,
                controller: placement.player,
                ..FigureState::default()
            });
        }
        state.units.push(unit_state);
    }
    Ok(state)
}

/// Resolves a handle such as `p0.soldier.trooper#2` or `p1.dummy#1` to a figure.
pub(super) fn figure(
    state: &GameState,
    registry: &ContentRegistry,
    handle: &str,
) -> Result<FigureId, String> {
    let bad = || format!("`{handle}` is not a figure handle like `p0.unit.figure#1`");
    let (path, nth) = handle.rsplit_once('#').ok_or_else(bad)?;
    let nth: usize = nth.parse().ok().filter(|n| *n >= 1).ok_or_else(bad)?;
    let parts: Vec<&str> = path.split('.').collect();
    let (player, unit, entry) = match parts[..] {
        [p, unit] => (p, unit, None),
        [p, unit, entry] => (p, unit, Some(entry)),
        _ => return Err(bad()),
    };
    let player: u8 = player
        .strip_prefix('p')
        .and_then(|n| n.parse().ok())
        .ok_or_else(bad)?;
    let matches = (0u32..)
        .zip(&state.figures)
        .filter(|(_, f)| f.controller == PlayerId(player))
        .filter(|(_, f)| entry.is_none_or(|e| f.def.figure.as_str() == e))
        .filter(|(_, f)| {
            let full = registry.name::<UnitDef>(f.def.unit).unwrap_or_default();
            full == unit || full.split_once(':').is_some_and(|(_, bare)| bare == unit)
        });
    let found = matches.map(|(id, _)| FigureId(id)).nth(nth - 1);
    found.ok_or_else(|| format!("no figure matches `{handle}`"))
}
