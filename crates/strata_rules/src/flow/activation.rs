//! A unit's activation: steps, per-figure slot budgets and ordering (design §6.5, §12.4).

use std::collections::BTreeMap;

use strata_kernel::Sym;
use strata_model::{Activation, Event, FigureId, FigureStatus, PlayerId, UnitId};

use crate::engine::{Ctx, Halt};

/// How figures and steps interleave during an activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderingPolicy {
    /// Every figure spends the current step's slots before the next step opens.
    AllFiguresPerStep,
    /// One figure spends all of its slots, in step order, before another may start.
    PerFigure,
}

/// The activation module's configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivationRules {
    /// The steps in order. Each is also the name of the slot it grants.
    pub steps: Vec<Sym>,
    /// Slots per figure for a step or an extra slot; steps default to one.
    pub budgets: BTreeMap<Sym, u8>,
    /// How figures and steps interleave.
    pub ordering: OrderingPolicy,
}

impl ActivationRules {
    fn budget(&self) -> BTreeMap<Sym, u8> {
        let mut all: BTreeMap<Sym, u8> = self.steps.iter().map(|s| (s.clone(), 1)).collect();
        all.extend(self.budgets.clone());
        all
    }

    fn step_index(&self, slot: &Sym) -> Option<usize> {
        self.steps.iter().position(|s| s == slot)
    }

    /// Whether a figure that has spent part of its budget is mid-way through its turn.
    fn started(&self, slots: &BTreeMap<Sym, u8>) -> bool {
        let spent = |(s, left): (&Sym, &u8)| self.budget().get(s).is_some_and(|b| left < b);
        slots.iter().any(spent) && slots.values().any(|&n| n > 0)
    }

    /// The slots `figure` may spend right now, with how many it has left.
    pub fn open(&self, act: &Activation, figure: FigureId) -> BTreeMap<Sym, u8> {
        let Some(slots) = act.slots.get(&figure) else {
            return BTreeMap::new();
        };
        let busy = act
            .slots
            .iter()
            .any(|(f, s)| *f != figure && self.started(s));
        let lowest = slots
            .iter()
            .filter(|(_, n)| **n > 0)
            .filter_map(|(s, _)| self.step_index(s))
            .min();
        let allowed = |slot: &Sym| match (self.step_index(slot), self.ordering) {
            (None, _) => true,
            (Some(i), OrderingPolicy::AllFiguresPerStep) => self.step_index(&act.step) == Some(i),
            (Some(i), OrderingPolicy::PerFigure) => lowest == Some(i),
        };
        let blocked = self.ordering == OrderingPolicy::PerFigure && busy;
        slots
            .iter()
            .filter(|(s, n)| **n > 0 && !blocked && allowed(s))
            .map(|(s, &n)| (s.clone(), n))
            .collect()
    }

    /// Whether `figure` can spend every slot in `needed` now.
    pub fn can_pay(&self, act: &Activation, figure: FigureId, needed: &[Sym]) -> bool {
        let open = self.open(act, figure);
        needed.iter().all(|slot| {
            let want = needed.iter().filter(|s| *s == slot).count();
            open.get(slot).is_some_and(|&n| usize::from(n) >= want)
        })
    }

    /// Starts `player`'s activation of `unit`: one budget per figure on the board.
    pub fn begin(&self, ctx: &mut dyn Ctx, player: PlayerId, unit: UnitId) -> Result<(), Halt> {
        let state = ctx.state();
        let Some(u) = state.units.get(unit.index()).filter(|u| u.owner == player) else {
            return Err(Halt::reject("not your unit"));
        };
        let on_board = |f: &&FigureId| {
            let fig = state.figures.get(f.index());
            fig.is_some_and(|f| f.status == FigureStatus::OnBoard)
        };
        let slots: BTreeMap<_, _> = u
            .figures
            .iter()
            .filter(on_board)
            .map(|f| (*f, self.budget()))
            .collect();
        let step = self.steps.first().ok_or_else(|| Halt::reject("no steps"))?;
        if slots.is_empty() {
            return Err(Halt::reject("the unit has no figures on the board"));
        }
        let step = step.clone();
        ctx.emit(Event::ActivationStarted { unit, step, slots })
    }

    /// Spends `needed` from `figure`, then opens the next step once this one is used up.
    pub fn consume(&self, ctx: &mut dyn Ctx, figure: FigureId, needed: &[Sym]) -> Result<(), Halt> {
        let act = ctx.state().flow.activation.clone();
        let Some(act) = act.filter(|a| self.can_pay(a, figure, needed)) else {
            return Err(Halt::reject("the figure cannot spend those slots now"));
        };
        let mut left = act.slots.get(&figure).cloned().unwrap_or_default();
        for slot in needed {
            let n = left.entry(slot.clone()).or_default();
            *n -= 1;
            let remaining = *n;
            let slot = slot.clone();
            ctx.emit(Event::SlotsChanged {
                figure,
                slot,
                remaining,
            })?;
        }
        self.settle(ctx)
    }

    /// Gives `figure` `extra` more of `slot`. A slot from an earlier step reopens that step.
    pub fn grant(
        &self,
        ctx: &mut dyn Ctx,
        figure: FigureId,
        slot: &Sym,
        extra: u8,
    ) -> Result<(), Halt> {
        let act = ctx.state().flow.activation.clone();
        let Some((act, held)) = act.and_then(|a| {
            let held = a.slots.get(&figure)?.get(slot).copied().unwrap_or(0);
            Some((a, held))
        }) else {
            return Err(Halt::reject("the figure is not in this activation"));
        };
        let remaining = held
            .checked_add(extra)
            .ok_or_else(|| Halt::reject("too many slots"))?;
        ctx.emit(Event::SlotsChanged {
            figure,
            slot: slot.clone(),
            remaining,
        })?;
        let reopens = match (self.step_index(slot), self.step_index(&act.step)) {
            (Some(slot_at), Some(now)) => slot_at < now,
            _ => false,
        };
        if reopens && self.ordering == OrderingPolicy::AllFiguresPerStep {
            ctx.emit(Event::StepChanged(slot.clone()))?;
        }
        Ok(())
    }

    /// Moves to the next step that still has slots once the current one has none.
    fn settle(&self, ctx: &mut dyn Ctx) -> Result<(), Halt> {
        let Some(act) = ctx.state().flow.activation.clone() else {
            return Ok(());
        };
        let left = |step: &Sym| {
            act.slots
                .values()
                .any(|s| s.get(step).is_some_and(|&n| n > 0))
        };
        let here = self.step_index(&act.step).unwrap_or(0);
        if self.ordering == OrderingPolicy::PerFigure || left(&act.step) {
            return Ok(());
        }
        match self.steps.iter().skip(here + 1).find(|s| left(s)) {
            Some(next) => ctx.emit(Event::StepChanged(next.clone())),
            None => Ok(()),
        }
    }
}
