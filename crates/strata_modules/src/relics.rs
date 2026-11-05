//! `core:relics`: relic spots, claiming, reveal and duration (design §6.1, §10.6).
//!
//! A relic is claimed when a figure's move ends on its spot (`claim = "ends_move_on"`). A
//! `while_occupied` relic follows its occupant: its abilities are attached to the occupant's army
//! while they stand on it and detached the moment they leave, are destroyed or lose control of
//! the figure. An `on_claim` relic runs its effect once, for the first player to end a move on
//! it. A face-down relic is turned up when it is claimed.

use std::collections::BTreeMap;

use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{
    AbilityKey, AbilityOwner, Descriptor, Event, FigureStatus, GameState, Logic, PlayerId,
    RelicDef, RelicId, RelicMode, TriggerDef,
};
use strata_rules::{
    ConfigSchema, Ctx, Halt, ModuleError, NativeEnv, NativeLogic, Registrar, RuleModule,
};

use crate::toy::no_config;

/// The only claim rule: a move that ends on the relic's spot.
const ENDS_MOVE_ON: &str = "ends_move_on";
/// The only recipient of a grant: the army of whoever holds the relic.
const OCCUPANT_ARMY: &str = "occupant_army";
/// The name the driver is registered under.
const DRIVE: &str = "core:relics.drive";
/// The events after which who stands on a relic may have changed.
const WATCHED: [&str; 3] = ["figure_moved", "figure_destroyed", "control_changed"];

/// The relics slot.
pub struct Relics;

impl RuleModule for Relics {
    fn id(&self) -> ModuleId {
        ModuleId::new("core:relics")
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("relics"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        no_config()
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.native(DRIVE, Box::new(Drive));
        let call = [(Sym::new("native"), Value::Str(Sym::new(DRIVE)))];
        for event in WATCHED {
            r.trigger(TriggerDef {
                event: Sym::new(event),
                filter: None,
                limit: None,
                effect: Logic::Native(Descriptor(Value::Map(call.clone().into()))),
            });
        }
        Ok(())
    }
}

/// Re-checks every relic's occupant after a figure moved, died or changed hands.
struct Drive;

impl NativeLogic for Drive {
    fn run(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, event: &Event) -> Result<(), Halt> {
        for index in 0..ctx.state().relics.len() {
            let id = RelicId(index as u32);
            let def = env.relic(ctx.state().relics[index].def);
            let Some(def) = def.filter(|d| d.claim.as_str() == ENDS_MOVE_ON) else {
                continue;
            };
            let occupant = occupant(ctx.state(), id);
            match def.mode {
                RelicMode::WhileOccupied => follow(ctx, id, &def, occupant)?,
                RelicMode::OnClaim => claim_once(ctx, env, id, &def, occupant, event)?,
            }
        }
        Ok(())
    }
}

/// The player controlling the figure on the relic's spot, if there is one.
fn occupant(state: &GameState, relic: RelicId) -> Option<PlayerId> {
    let at = state.relics[relic.index()].location?;
    let figure = state.board_changes.occupancy.get(&at)?;
    let figure = &state.figures[figure.index()];
    (figure.status == FigureStatus::OnBoard).then_some(figure.controller)
}

/// Gives a `while_occupied` relic to `occupant`, taking it from whoever held it before.
fn follow(
    ctx: &mut dyn Ctx,
    relic: RelicId,
    def: &RelicDef,
    occupant: Option<PlayerId>,
) -> Result<(), Halt> {
    let state = ctx.state();
    let held = state.relics[relic.index()].holder;
    if held == occupant {
        return Ok(());
    }
    let relic_def = state.relics[relic.index()].def;
    let key = |index: u16| AbilityKey::Relic {
        relic: relic_def,
        index,
    };
    if let Some(old) = held {
        for grant in grants(def) {
            let owner = AbilityOwner::Player(old);
            ctx.emit(Event::AbilityDetached {
                def: key(grant),
                owner,
            })?;
        }
        ctx.emit(Event::RelicReleased(relic))?;
    }
    let Some(new) = occupant else {
        return Ok(());
    };
    claim(ctx, relic, new)?;
    for grant in grants(def) {
        ctx.emit(Event::AbilityAttached {
            def: key(grant),
            owner: AbilityOwner::Player(new),
            params: BTreeMap::new(),
        })?;
    }
    Ok(())
}

/// Claims an unclaimed `on_claim` relic for its occupant and runs its effect.
fn claim_once(
    ctx: &mut dyn Ctx,
    env: &dyn NativeEnv,
    relic: RelicId,
    def: &RelicDef,
    occupant: Option<PlayerId>,
    event: &Event,
) -> Result<(), Halt> {
    let Some(player) = occupant.filter(|_| ctx.state().relics[relic.index()].holder.is_none())
    else {
        return Ok(());
    };
    claim(ctx, relic, player)?;
    match &def.effect {
        Some(effect) => env.run_effect(ctx, effect, event),
        None => Ok(()),
    }
}

/// Turns the relic up if it is face down, then makes `player` its holder.
fn claim(ctx: &mut dyn Ctx, relic: RelicId, player: PlayerId) -> Result<(), Halt> {
    if ctx.state().relics[relic.index()].face_down {
        ctx.emit(Event::RelicRevealed(relic))?;
    }
    ctx.emit(Event::RelicClaimed { relic, by: player })
}

/// The positions of the relic's granted abilities. Only grants to the occupant's army exist.
/// `AbilityKey` indexes entries with a `u16`, so a relic attaches at most its first 65,535
/// grants; no real relic comes near that.
fn grants(def: &RelicDef) -> std::ops::Range<u16> {
    match &def.grants {
        Some(g) if g.to.as_str() == OCCUPANT_ARMY => {
            0..u16::try_from(g.abilities.len()).unwrap_or(u16::MAX)
        }
        _ => 0..0,
    }
}
