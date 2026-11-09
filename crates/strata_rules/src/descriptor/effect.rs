//! Running effect descriptors (design §8.2).
//!
//! An effect changes the game, so unlike a selector it does not evaluate to a value: it emits
//! events through the [`Ctx`] of the resolution it runs in. Effects that a rules slot owns go
//! through that slot's exported functions, so the slot's rules still apply: wounds, healing and
//! removal call `ctx.units`, and the combat effects read and change the attack in progress through
//! `ctx.combat`. The rest emit their event directly.

use std::collections::BTreeMap;

use strata_kernel::{Hex, Space, Sym, Value};
use strata_model::{
    Answer, Decision, DecisionKind, Event, FigureId, FigureStatus, GameState, PlayerId,
};

use super::{Base, Eff, NativeEvaluator, Sel, Val, World};
use crate::engine::{Ctx, Halt, event_subjects};
use crate::native::NativeEnv;
use crate::query::{EvalCtx, LogicError};

/// What an effect needs from the host it runs under.
pub trait EffectHost {
    /// What the loaded content answers, such as marker IDs and footprint cells.
    fn env(&self) -> &dyn NativeEnv;
    /// The game as descriptors see it, for selectors and values.
    fn world<'a>(&'a self, state: &'a GameState) -> Result<Box<dyn World + 'a>, LogicError>;
    /// Calls `ctx.<namespace>.<name>` of a module that exported it from Rust.
    fn api(
        &self,
        ctx: &mut dyn Ctx,
        namespace: &str,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Halt>;
}

/// Whom an effect is about.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectCx<'a> {
    /// What `self` means: a figure ID, or nil when nothing owns the hook.
    pub subject: Value,
    /// What `target` means: the figure the effect is aimed at, if any.
    pub target: Option<FigureId>,
    /// The event a replacement is running in place of, if the effect is a replacement.
    pub replacing: Option<&'a Event>,
}

impl EffectCx<'_> {
    /// The context of a trigger, replacement or relic effect for `event`, which has no owner of
    /// its own: `self` and `target` are the figures [`event_subjects`] reads from the event.
    pub fn for_event(state: &GameState, event: &Event) -> Self {
        let (own, target) = event_subjects(state, event);
        EffectCx {
            subject: own.map_or(Value::Nil, id),
            target,
            replacing: None,
        }
    }
}

fn reject<T>(message: impl Into<String>) -> Result<T, Halt> {
    Err(Halt::reject(message.into()))
}

fn logic(e: LogicError) -> Halt {
    Halt::reject(e.0)
}

/// Whether a figure is still in play.
fn live(state: &GameState, figure: FigureId) -> bool {
    let found = state.figures.get(figure.index());
    found.is_some_and(|f| matches!(f.status, FigureStatus::Reserve | FigureStatus::OnBoard))
}

fn id(figure: FigureId) -> Value {
    Value::Int(i64::from(figure.0))
}

/// Runs `eff` for `cx`, emitting its events through `ctx`. Figures that leave play while an
/// effect works through its list are skipped.
pub fn run_effect(
    eff: &Eff,
    ctx: &mut dyn Ctx,
    host: &dyn EffectHost,
    cx: &EffectCx<'_>,
) -> Result<(), Halt> {
    let run = Run { host, cx };
    match eff {
        Eff::Wound { who, amount } => run.units(ctx, who, Some(amount), "wound"),
        Eff::Heal { who, amount } => run.units(ctx, who, Some(amount), "heal"),
        Eff::Remove { who } => run.units(ctx, who, None, "remove"),
        Eff::Place { who, within, of } => {
            let radius = run.count(ctx, within, "within")?;
            let world = host.world(ctx.state()).map_err(logic)?;
            let anchors = run.select(&*world, of)?;
            let anchors: Vec<Hex> = anchors
                .iter()
                .filter_map(|f| world.location(*f))
                .map(Space::hex)
                .collect();
            drop(world);
            for figure in run.pick(ctx, who)? {
                run.relocate(ctx, figure, &anchors, radius, false)?;
            }
            Ok(())
        }
        Eff::Move { who, up_to } => {
            let radius = run.count(ctx, up_to, "up_to")?;
            for figure in run.pick(ctx, who)? {
                let Some(at) = ctx.state().standing(figure) else {
                    continue;
                };
                run.relocate(ctx, figure, &[at.hex()], radius, true)?;
            }
            Ok(())
        }
        Eff::GrantSlot { slot, count, who } => {
            let more = u8::try_from(run.count(ctx, count, "count")?).unwrap_or(u8::MAX);
            for figure in run.pick(ctx, who)? {
                let activation = ctx.state().flow.activation.as_ref();
                let Some(slots) = activation.and_then(|a| a.slots.get(&figure)) else {
                    continue;
                };
                let have = slots.get(slot).copied().unwrap_or(0);
                let remaining = have.saturating_add(more);
                let slot = slot.clone();
                ctx.emit(Event::SlotsChanged {
                    figure,
                    slot,
                    remaining,
                })?;
            }
            Ok(())
        }
        Eff::AddMarker {
            marker,
            count,
            rounds,
            who,
        } => {
            let Some(def) = host.env().marker(marker.as_str()) else {
                return reject(format!("the content has no `{marker}` marker"));
            };
            let delta = i32::try_from(run.count(ctx, count, "count")?).unwrap_or(i32::MAX);
            let round = ctx.state().flow.round;
            let expiry = rounds.map(|r| round.saturating_add(r - 1));
            let source = run.subject();
            for figure in run.pick(ctx, who)? {
                if delta > 0 && live(ctx.state(), figure) {
                    let event = Event::MarkerChanged {
                        figure,
                        marker: def,
                        delta,
                        source,
                        expiry,
                    };
                    ctx.emit(event)?;
                }
            }
            Ok(())
        }
        Eff::TakeControl { who } => {
            let player = run.chooser(ctx.state());
            for figure in run.pick(ctx, who)? {
                let state = ctx.state();
                if live(state, figure) && state.figures[figure.index()].controller != player {
                    ctx.emit(Event::ControlChanged {
                        figure,
                        controller: player,
                    })?;
                }
            }
            Ok(())
        }
        Eff::CancelWounds => {
            if let Some(Event::WoundsApplied { .. }) = cx.replacing {
                // The replacement emits nothing in the event's place, which cancels it.
                return Ok(());
            }
            // The attack keeps its hits and blocks, so excess blocks still count; the combat
            // slot applies no wounds. In `combat.after_wounds` the wounds are already applied,
            // and the combat slot refuses.
            run.attack(ctx, "cancel_wounds")?;
            host.api(ctx, "combat", "cancel", &[]).map(drop)
        }
        Eff::ExcessBlocksWoundAttacker => {
            let (hits, blocks, attacker) = run.attack(ctx, "excess_blocks_wound_attacker")?;
            let excess = blocks - hits;
            if excess > 0 && live(ctx.state(), attacker) {
                host.api(ctx, "units", "wound", &[id(attacker), Value::Int(excess)])?;
            }
            Ok(())
        }
    }
}

struct Run<'a> {
    host: &'a dyn EffectHost,
    cx: &'a EffectCx<'a>,
}

impl Run<'_> {
    fn eval_cx<'s>(&'s self, source: &'s Sym, context: &'s Value) -> EvalCtx<'s> {
        EvalCtx {
            source,
            subject: &self.cx.subject,
            context,
        }
    }

    fn context(&self) -> Value {
        self.cx.target.map_or(Value::Nil, id)
    }

    /// The figures of `sel`. With no subject, as for an event about no figure, a selector that
    /// starts from `self` selects nothing rather than failing, so the effect does nothing.
    fn select(&self, world: &dyn World, sel: &Sel) -> Result<Vec<FigureId>, Halt> {
        if self.cx.subject.is_nil() && sel.base != Base::Target {
            return Ok(Vec::new());
        }
        let (source, context) = (Sym::new("effect"), self.context());
        let found = NativeEvaluator { world }.select(sel, &self.eval_cx(&source, &context));
        found.map_err(logic)
    }

    /// The figures of `sel` in the current state.
    fn pick(&self, ctx: &dyn Ctx, sel: &Sel) -> Result<Vec<FigureId>, Halt> {
        let world = self.host.world(ctx.state()).map_err(logic)?;
        self.select(&*world, sel)
    }

    /// The value of `val`, which must not be negative.
    fn count(&self, ctx: &dyn Ctx, val: &Val, what: &str) -> Result<u32, Halt> {
        let world = self.host.world(ctx.state()).map_err(logic)?;
        let (source, context) = (Sym::new("effect"), self.context());
        let n = NativeEvaluator { world: &*world }.value(val, &self.eval_cx(&source, &context));
        let n = n.map_err(logic)?;
        u32::try_from(n).or_else(|_| reject(format!("`{what}` must not be negative, not {n}")))
    }

    /// The subject, if it is a figure.
    fn subject(&self) -> Option<FigureId> {
        let n = self.cx.subject.as_int().ok()?;
        u32::try_from(n).ok().map(FigureId)
    }

    /// Who chooses for the effect and takes control: the subject's controller, or the active
    /// player when the subject is not a figure.
    fn chooser(&self, state: &GameState) -> PlayerId {
        let figure = self.subject().and_then(|f| state.figures.get(f.index()));
        figure.map_or(state.flow.active, |f| f.controller)
    }

    /// Calls `ctx.units.<name>` for each figure of `who` still in play, with the amount if any.
    fn units(
        &self,
        ctx: &mut dyn Ctx,
        who: &Sel,
        amount: Option<&Val>,
        name: &str,
    ) -> Result<(), Halt> {
        let amount = amount.map(|a| self.count(ctx, a, "amount")).transpose()?;
        if amount == Some(0) {
            return Ok(());
        }
        for figure in self.pick(ctx, who)? {
            if live(ctx.state(), figure) {
                let mut args = vec![id(figure)];
                args.extend(amount.map(|n| Value::Int(i64::from(n))));
                self.host.api(ctx, "units", name, &args)?;
            }
        }
        Ok(())
    }

    /// The hits, blocks and attacker of the attack in progress.
    fn attack(&self, ctx: &mut dyn Ctx, what: &str) -> Result<(i64, i64, FigureId), Halt> {
        let current = self.host.api(ctx, "combat", "current", &[])?;
        if current.is_nil() {
            return reject(format!("eff.{what} needs an attack in progress"));
        }
        let bad = |e: strata_kernel::ValueError| Halt::reject(format!("eff.{what}: {e}"));
        let int = |key: &str| current.get(key).and_then(Value::as_int).map_err(bad);
        let attacker = u32::try_from(int("attacker")?).map_err(|_| Halt::reject("bad attacker"));
        Ok((int("hits")?, int("blocks")?, FigureId(attacker?)))
    }

    /// Asks the chooser for an empty space within `radius` of any of `anchors` and puts
    /// `figure` there. A move leaves its own space out and records the jump as its path; a
    /// placement has no path. Either is a jump: it ignores climbing limits and `can_enter` votes,
    /// and provokes no reprisals. Nothing happens when no space is open.
    fn relocate(
        &self,
        ctx: &mut dyn Ctx,
        figure: FigureId,
        anchors: &[Hex],
        radius: u32,
        moving: bool,
    ) -> Result<(), Halt> {
        if !live(ctx.state(), figure) {
            return Ok(());
        }
        let open = self.open_spaces(ctx.state(), figure, anchors, radius, moving)?;
        if open.is_empty() {
            return Ok(());
        }
        let decision = Decision {
            prompt: Sym::new(if moving { "move" } else { "place" }),
            kind: DecisionKind::ChooseSpace(open.keys().copied().collect()),
        };
        let chooser = self.chooser(ctx.state());
        let Answer::Space(to) = ctx.choose(chooser, decision)? else {
            return reject("answer does not fit the decision");
        };
        let Some(cells) = open.get(&to).cloned() else {
            return reject("that space is not open");
        };
        let rotation = ctx.state().figures[figure.index()].rotation;
        let path = if moving { vec![to] } else { Vec::new() };
        ctx.emit(Event::FigureMoved {
            figure,
            to,
            rotation,
            path,
            cells,
        })
    }

    /// Every standable space within `radius` of an anchor where `figure`'s footprint fits
    /// without overlapping another figure, with the cells it would cover.
    fn open_spaces(
        &self,
        state: &GameState,
        figure: FigureId,
        anchors: &[Hex],
        radius: u32,
        moving: bool,
    ) -> Result<BTreeMap<Space, Vec<Space>>, Halt> {
        let (env, world) = (self.host.env(), self.host.world(state).map_err(logic)?);
        let rotation = state.figures[figure.index()].rotation;
        let here = state.standing(figure);
        let hexes: std::collections::BTreeSet<Hex> =
            anchors.iter().flat_map(|a| a.spiral(radius)).collect();
        let mut open = BTreeMap::new();
        for hex in hexes {
            for z in world.levels(hex) {
                let space = Space::new(hex.q, hex.r, z);
                if (moving && here == Some(space)) || !env.standable(space) {
                    continue;
                }
                let Some(cells) = env.cells(state, figure, space, rotation) else {
                    continue;
                };
                let free = |c: &Space| state.occupant(*c).is_none_or(|o| o == figure);
                if cells.iter().all(free) {
                    open.insert(space, cells);
                }
            }
        }
        Ok(open)
    }
}

#[cfg(test)]
mod tests;
