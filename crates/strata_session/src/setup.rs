//! Match setup: the fixed inputs become the first `GameState` and the compiled board
//! (design §12.2).

use std::collections::BTreeSet;
use std::sync::Arc;

use strata_content::ContentRegistry;
use strata_kernel::{DefId, Space};
use strata_model::board::{CompiledBoard, compile};
use strata_model::{
    ArmyList, FigureDefRef, FigureId, FigureState, GameState, MapDef, MatchSetup, PackIdentity,
    PlayerId, PlayerState, RelicDef, RelicState, ScenarioDef, UnitDef, UnitId, UnitState,
};

use crate::pack_test::host::{RegistryDefs, find};

/// The setup cannot be turned into a match.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct SetupError(pub String);

/// A match ready to play: its first state and the board that state stands on.
pub struct MatchBuild {
    /// The state before the first command.
    pub state: GameState,
    /// The compiled map, which the state does not carry.
    pub board: CompiledBoard,
}

/// The identity of every pack in `registry`, as a [`MatchSetup`] records it.
pub fn pack_identities(registry: &ContentRegistry) -> Vec<PackIdentity> {
    let identity = |p: &strata_content::PackIdentity| {
        let [a, b, c, d, e, f, g, h, ..] = p.content_hash.0;
        PackIdentity {
            id: p.id.clone(),
            version: p.version.to_string(),
            hash: u64::from_be_bytes([a, b, c, d, e, f, g, h]),
        }
    };
    registry.pack_identities().iter().map(identity).collect()
}

/// Builds the state a match starts from: army lists deployed into reserve, relics put on their
/// spots, and the scenario's map compiled.
pub fn build_match(
    registry: &ContentRegistry,
    setup: MatchSetup,
) -> Result<MatchBuild, SetupError> {
    let err = |m: String| SetupError(m);
    let scenario = registry
        .get::<ScenarioDef>(setup.scenario)
        .ok_or_else(|| err("the scenario is not in the registry".into()))?;
    let full = registry.name::<ScenarioDef>(setup.scenario);
    let pack = full.and_then(|n| n.split_once(':')).map_or("", |(p, _)| p);
    let (_, map) = find::<MapDef>(registry, pack, scenario.map.as_str())
        .ok_or_else(|| err(format!("unknown map `{}`", scenario.map)))?;
    let defs = RegistryDefs { registry, pack };
    let board = compile(map, &defs).map_err(|e| err(e.to_string()))?;
    check_seats(&setup, scenario)?;
    let mut state = GameState::default();
    for (seat, player) in setup.players.iter().enumerate() {
        state.players.push(PlayerState {
            team: player.team,
            hand: Vec::new(),
        });
        if let Some(army) = &player.army {
            deploy(&mut state, registry, pack, PlayerId(seat as u8), army)?;
        }
    }
    for placed in &scenario.relics {
        let (id, _) = find::<RelicDef>(registry, pack, placed.relic.as_str())
            .ok_or_else(|| err(format!("unknown relic `{}`", placed.relic)))?;
        let hex = map.relic_spots.get(&placed.spot);
        let hex = hex.ok_or_else(|| err(format!("unknown relic spot `{}`", placed.spot)))?;
        let top = board.top_surface(*hex);
        let top = top.ok_or_else(|| err(format!("relic spot `{}` is off the map", placed.spot)))?;
        state.relics.push(RelicState {
            def: id,
            location: Some(Space::at(*hex, top.z)),
            ..RelicState::default()
        });
    }
    state.setup = Arc::new(setup);
    Ok(MatchBuild { state, board })
}

/// Checks the seat count, that `players[i]` has `PlayerId(i)`, and that slots are real and unique.
fn check_seats(setup: &MatchSetup, scenario: &ScenarioDef) -> Result<(), SetupError> {
    let err = |m: String| SetupError(m);
    let count = setup.players.len();
    let (min, max) = (scenario.players.min, scenario.players.max);
    if count < usize::from(min) || count > usize::from(max) {
        return Err(err(format!(
            "the scenario takes {min} to {max} players, not {count}"
        )));
    }
    let mut taken = BTreeSet::new();
    for (seat, player) in setup.players.iter().enumerate() {
        if player.id.index() != seat {
            return Err(err(format!("player {} sits at seat {seat}", player.id.0)));
        }
        if !scenario.slots.iter().any(|s| s.id == player.slot) {
            return Err(err(format!("the scenario has no slot `{}`", player.slot)));
        }
        if !taken.insert(&player.slot) {
            return Err(err(format!("slot `{}` is filled twice", player.slot)));
        }
    }
    Ok(())
}

/// Adds every unit of `army` to `state`, its figures waiting in reserve.
fn deploy(
    state: &mut GameState,
    registry: &ContentRegistry,
    pack: &str,
    owner: PlayerId,
    army: &ArmyList,
) -> Result<(), SetupError> {
    for entry in &army.units {
        let name = entry.unit.as_str();
        let (def_id, def) = find::<UnitDef>(registry, pack, name)
            .ok_or_else(|| SetupError(format!("army: unknown unit `{name}`")))?;
        for copy in 0..entry.copies {
            add_unit(state, def_id, def, owner, copy);
        }
    }
    Ok(())
}

fn add_unit(state: &mut GameState, def_id: DefId, def: &UnitDef, owner: PlayerId, copy: u8) {
    let unit = UnitId(state.units.len() as u32);
    let mut figures = Vec::new();
    for entry in &def.figures {
        for _ in 0..entry.count {
            let id = FigureId(state.figures.len() as u32);
            figures.push(id);
            state.figures.push(FigureState {
                unit,
                def: FigureDefRef {
                    unit: def_id,
                    figure: entry.id.clone(),
                },
                controller: owner,
                ..FigureState::default()
            });
        }
    }
    state.units.push(UnitState {
        def: def_id,
        owner,
        copy,
        figures,
        ..UnitState::default()
    });
}
