//! `core:command_tokens`: numbered tokens and decoys that decide which unit acts (design §6.1,
//! §12.5).
//!
//! At the start of the `token_placement` phase each player is dealt tokens numbered `1..=numbered`
//! plus `decoys` decoys (fewer if they have fewer units than tokens) and secretly puts each on a
//! different unit of theirs. In the `turns` phase, a player's n-th turn reveals their token
//! number n, and only the unit holding it may be activated. A player whose token's unit is gone
//! loses that turn. Cards are cleared when the round ends, and when a unit is removed.
//!
//! Token kinds are plain numbers: [`DECOY`] is 0 and [`numbered`]`(n)` is `n`.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use strata_kernel::{DefId, ModuleId, SlotId, Sym, Value};
use strata_model::{
    Command, ConfigField, Descriptor, Event, FigureStatus, GameState, Logic, PlayerId, Token,
    TriggerDef, UnitId, VarKey, Visibility,
};
use strata_rules::{
    ActivationGate, Anchor, CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, NativeEnv,
    NativeLogic, PhaseDef, Registrar, RuleModule,
};

use crate::rounds::{self, PHASE_OPENED, ROUND_ENDED, TURN_SKIPPED};
use crate::units::UNIT_REMOVED;

/// The phase in which players place their tokens.
pub const PLACEMENT: &str = "token_placement";
/// The kind of a decoy, which never activates anything.
pub const DECOY: DefId = DefId(0);

/// The custom event emitted once every player's tokens are on cards.
const PLACEMENT_DONE: &str = "tokens_placed";
/// The name the driver is registered under.
const DRIVE: &str = "core:command_tokens.drive";
/// The module that owns the variables.
const OWNER: &str = "core:command_tokens";

/// The kind of the token that decides a player's `n`-th turn.
pub fn numbered(n: u32) -> DefId {
    DefId(n)
}

/// The turns slot's partner: command tokens in the `tokens` slot.
pub struct CommandTokens;

impl RuleModule for CommandTokens {
    fn id(&self) -> ModuleId {
        ModuleId::new(OWNER)
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("tokens"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        static NEEDS: OnceLock<[ModuleId; 1]> = OnceLock::new();
        NEEDS.get_or_init(|| [ModuleId::new("core:rounds")])
    }

    fn config_schema(&self) -> &ConfigSchema {
        static SCHEMA: OnceLock<ConfigSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            let field = |default| ConfigField {
                kind: Sym::new("int"),
                default: Value::Int(default),
            };
            let fields = [("numbered", field(3)), ("decoys", field(1))];
            ConfigSchema::new(fields.map(|(k, v)| (Sym::new(k), v)).into())
        })
    }

    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError> {
        let count = |name: &str| {
            let found = cfg.get(name).and_then(Value::as_int);
            let found = found.map_err(|e| ModuleError::Invalid(e.to_string()))?;
            u32::try_from(found).map_err(|_| ModuleError::Invalid(format!("{name} is negative")))
        };
        let drive = Drive {
            numbered: count("numbered")?,
            decoys: count("decoys")?,
        };
        r.phase(
            PhaseDef {
                id: Sym::new(PLACEMENT),
            },
            Anchor::After(Sym::new(rounds::INITIATIVE)),
        );
        r.command(Place);
        r.native(DRIVE, Box::new(drive));
        r.service::<dyn ActivationGate>(Box::new(Gate));
        let call = [(Sym::new("native"), Value::Str(Sym::new(DRIVE)))];
        let events = [
            PHASE_OPENED,
            PLACEMENT_DONE,
            "turn_started",
            ROUND_ENDED,
            UNIT_REMOVED,
        ];
        for event in events {
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

fn key(name: &str) -> VarKey {
    VarKey {
        module: ModuleId::new(OWNER),
        name: Sym::new(name),
    }
}

/// Which of a player's turns `turn` is, counted from 1. Players alternate, so with `seats`
/// players every `seats`-th turn is the same player's next one.
fn nth_turn(state: &GameState, turn: u32) -> u32 {
    let seats = state.players.len().max(1) as u32;
    turn.saturating_sub(1) / seats + 1
}

fn is_live(state: &GameState, unit: usize) -> bool {
    let live = |f: &strata_model::FigureId| {
        let status = state.figures.get(f.index()).map(|f| f.status);
        matches!(status, Some(FigureStatus::Reserve | FigureStatus::OnBoard))
    };
    state.units[unit].figures.iter().any(live)
}

/// Lets only the unit that holds the revealed token of the current turn act.
struct Gate;

impl ActivationGate for Gate {
    fn may_activate(&self, state: &GameState, player: PlayerId, unit: UnitId) -> bool {
        let want = numbered(nth_turn(state, state.flow.turn));
        let Some(u) = state.units.get(unit.index()).filter(|u| u.owner == player) else {
            return false;
        };
        let shown = |t: &Token| t.kind == want && t.visibility == Visibility::Public;
        u.tokens.iter().any(shown)
    }
}

/// Handles `place_tokens`: a player secretly puts their whole hand on distinct units.
struct Place;

impl CommandHandler for Place {
    fn kind(&self) -> &str {
        "place_tokens"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, player: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::PlaceTokens(put) = command else {
            return Err(Halt::reject("`place_tokens` takes token placements"));
        };
        let state = ctx.state();
        if state.vars.get(&key("placing")) != Some(&Value::Bool(true)) {
            return Err(Halt::reject("tokens are not being placed now"));
        }
        let mut hand = state.players[player.index()].hand.clone();
        if hand.is_empty() || put.len() != hand.len() {
            return Err(Halt::reject("place every token in your hand, once"));
        }
        let mut used = BTreeSet::new();
        for p in put {
            let mine = state.units.get(p.unit.index()).map(|u| u.owner) == Some(player);
            if !mine || !is_live(state, p.unit.index()) || !used.insert(p.unit) {
                return Err(Halt::reject(
                    "each token goes on a different live unit of yours",
                ));
            }
            let at = hand.iter().position(|k| *k == p.token);
            hand.remove(at.ok_or_else(|| Halt::reject("that token is not in your hand"))?);
        }
        for p in put {
            let token = Token {
                kind: p.token,
                visibility: Visibility::Hidden,
            };
            ctx.emit(Event::TokenPlaced {
                unit: p.unit,
                token,
            })?;
        }
        if ctx.state().players.iter().all(|p| p.hand.is_empty()) {
            return custom(ctx, PLACEMENT_DONE, Value::Nil);
        }
        Ok(())
    }
}

fn custom(ctx: &mut dyn Ctx, id: &str, payload: Value) -> Result<(), Halt> {
    let id = Sym::new(id);
    ctx.emit(Event::Custom { id, payload })
}

/// Deals, reveals and clears tokens as the round moves.
struct Drive {
    numbered: u32,
    decoys: u32,
}

impl NativeLogic for Drive {
    fn run(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, event: &Event) -> Result<(), Halt> {
        match event {
            Event::TurnStarted { turn, active } => reveal(ctx, *turn, *active),
            Event::Custom { id, payload } => match id.as_str() {
                PHASE_OPENED => match payload.get("phase").and_then(Value::as_str) {
                    Ok(PLACEMENT) => self.deal(ctx),
                    _ => Ok(()),
                },
                PLACEMENT_DONE => {
                    let tree = env.phases();
                    let tree = tree.ok_or_else(|| Halt::reject("tokens need the phase tree"))?;
                    let off = Event::VarSet {
                        key: key("placing"),
                        value: Value::Nil,
                    };
                    ctx.emit(off)?;
                    rounds::advance(ctx, tree)
                }
                ROUND_ENDED => {
                    let cards = ctx.state().units.len();
                    (0..cards).try_for_each(|u| discard(ctx, UnitId(u as u32)))
                }
                UNIT_REMOVED => {
                    let unit = payload.get("unit").and_then(Value::as_int);
                    let unit = unit.map_err(|e| Halt::reject(e.to_string()))?;
                    discard(ctx, UnitId(unit as u32))
                }
                _ => Ok(()),
            },
            _ => Ok(()),
        }
    }
}

impl Drive {
    /// Fills every hand: the numbered tokens first, then decoys, as many as the player has units.
    fn deal(&self, ctx: &mut dyn Ctx) -> Result<(), Halt> {
        let state = ctx.state();
        let mut hands = Vec::new();
        for seat in 0..state.players.len() {
            let player = PlayerId(seat as u8);
            let owned = (0..state.units.len()).filter(|u| state.units[*u].owner == player);
            let live = owned.filter(|u| is_live(state, *u)).count() as u32;
            let numbers = self.numbered.min(live);
            let decoys = self.decoys.min(live - numbers);
            let kinds = (1..=numbers).map(numbered);
            let tokens: Vec<DefId> = kinds.chain((0..decoys).map(|_| DECOY)).collect();
            hands.push((player, tokens));
        }
        let none_dealt = hands.iter().all(|(_, tokens)| tokens.is_empty());
        let on = Event::VarSet {
            key: key("placing"),
            value: Value::Bool(true),
        };
        ctx.emit(on)?;
        for (player, tokens) in hands {
            ctx.emit(Event::TokensDealt { player, tokens })?;
        }
        if none_dealt {
            custom(ctx, PLACEMENT_DONE, Value::Nil)?;
        }
        Ok(())
    }
}

/// Turns the active player's token for this turn face up, or passes the turn if no unit holds it.
fn reveal(ctx: &mut dyn Ctx, turn: u32, active: PlayerId) -> Result<(), Halt> {
    let state = ctx.state();
    let want = numbered(nth_turn(state, turn));
    let held = state
        .units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.owner == active)
        .find_map(|(i, u)| {
            let at = u.tokens.iter().position(|t| t.kind == want)?;
            Some((UnitId(i as u32), at as u8))
        });
    match held {
        Some((unit, index)) => ctx.emit(Event::TokenRevealed { unit, index }),
        None => custom(ctx, TURN_SKIPPED, Value::Nil),
    }
}

/// Takes every token off `unit`'s card, if it has any.
fn discard(ctx: &mut dyn Ctx, unit: UnitId) -> Result<(), Halt> {
    if ctx.state().units[unit.index()].tokens.is_empty() {
        return Ok(());
    }
    ctx.emit(Event::TokensDiscarded(unit))
}

#[cfg(test)]
mod tests;
