//! One run of a command against a working copy of the state (design §5.2, §5.3).

use std::collections::BTreeSet;

use strata_kernel::{CommandRng, ModuleId, Sym, Value, WindowId};
use strata_model::{
    Answer, Command, Decision, DecisionKind, Event, GameState, PlayerId, SeedSource, TriggerLimit,
    UnitId, VarKey,
};

use super::ctx::Ctx;
use super::halt::Halt;
use super::hooks::HookRunner;
use super::names::{affected_player, answer_fits, command_kind, event_kind, is_kind};
use crate::flow::ActivationGate;
use crate::native::native_name;
use crate::runtime::RulesRuntime;

mod window;

/// Decisions one command may ask before it is rejected as runaway.
pub const MAX_DECISIONS: usize = 64;
/// Events one command may emit before it is rejected as runaway.
const MAX_EVENTS: usize = 4096;

/// A trigger waiting to resolve.
struct Queued {
    index: usize,
    event: Event,
    player: PlayerId,
}

/// The [`Ctx`] for one run: a state clone, the command's dice and the answer tape.
pub(crate) struct Exec<'a> {
    rt: &'a RulesRuntime,
    hooks: &'a dyn HookRunner,
    state: GameState,
    rng: CommandRng,
    tape: &'a [Answer],
    cursor: usize,
    events: Vec<Event>,
    queue: Vec<Queued>,
    open_windows: Vec<WindowId>,
}

impl<'a> Exec<'a> {
    /// Runs `command` from the start with `tape`, returning the new state and its events.
    pub(crate) fn run(
        rt: &'a RulesRuntime,
        hooks: &'a dyn HookRunner,
        state: &GameState,
        player: PlayerId,
        command: &Command,
        tape: &'a [Answer],
    ) -> Result<(GameState, Vec<Event>), Halt> {
        let SeedSource::Plain(seed) = state.setup.seed;
        let kind = command_kind(command);
        let handler = rt
            .commands()
            .iter()
            .find(|h| h.kind() == kind)
            .ok_or_else(|| Halt::reject(format!("no handler for `{kind}`")))?;
        let mut state = state.clone();
        state.pending = None;
        let mut exec = Exec {
            rt,
            hooks,
            rng: CommandRng::for_command(seed, state.command_index),
            state,
            tape,
            cursor: 0,
            events: Vec::new(),
            queue: Vec::new(),
            open_windows: Vec::new(),
        };
        handler.resolve(&mut exec, player, command)?;
        exec.drain_triggers()?;
        Ok((exec.state, exec.events))
    }

    /// Runs `e` through the replacements not yet in `used`, then commits what is left.
    fn pipeline(&mut self, e: Event, used: &mut BTreeSet<usize>) -> Result<(), Halt> {
        let (rt, hooks) = (self.rt, self.hooks);
        let kind = event_kind(&e);
        let mut open = Vec::new();
        for (i, def) in rt.replacements().iter().enumerate() {
            if !used.contains(&i)
                && is_kind(&def.event, kind)
                && hooks.replaces(def, &e, &self.state)?
            {
                open.push(i);
            }
        }
        let pick = match open.as_slice() {
            [] => return self.commit(e),
            [only] => *only,
            _ => {
                let labels = open
                    .iter()
                    .map(|i| Sym::new(&format!("replacement:{i}")))
                    .collect();
                let player = affected_player(&self.state, &e);
                open[self.order("order_replacements", player, labels)?[0]]
            }
        };
        used.insert(pick);
        for next in hooks.replace(&rt.replacements()[pick], &e, self)? {
            self.pipeline(next, &mut used.clone())?;
        }
        Ok(())
    }

    /// Applies `e` and queues the triggers it sets off.
    fn commit(&mut self, e: Event) -> Result<(), Halt> {
        if self.events.len() >= MAX_EVENTS {
            return Err(Halt::reject("too many events in one command"));
        }
        self.state.apply(&e)?;
        let (rt, hooks) = (self.rt, self.hooks);
        let kind = event_kind(&e);
        for (index, def) in rt.triggers().iter().enumerate() {
            if is_kind(&def.event, kind)
                && self.within_limit(index)
                && hooks.fires(def, &e, &self.state)?
            {
                let player = affected_player(&self.state, &e);
                self.queue.push(Queued {
                    index,
                    event: e.clone(),
                    player,
                });
            }
        }
        self.events.push(e);
        Ok(())
    }

    /// Resolves queued triggers until none are left: active player first, each player's
    /// simultaneous triggers in the order that player chooses.
    fn drain_triggers(&mut self) -> Result<(), Halt> {
        while !self.queue.is_empty() {
            let batch = std::mem::take(&mut self.queue);
            let active = self.state.flow.active;
            let mut players: Vec<PlayerId> = batch.iter().map(|q| q.player).collect();
            players.sort();
            players.dedup();
            players.sort_by_key(|p| *p != active);
            for player in players {
                let mine: Vec<&Queued> = batch.iter().filter(|q| q.player == player).collect();
                let order = if mine.len() > 1 {
                    let labels = mine
                        .iter()
                        .map(|q| Sym::new(&format!("trigger:{}", q.index)))
                        .collect();
                    self.order("order_triggers", player, labels)?
                } else {
                    vec![0]
                };
                for i in order {
                    self.fire(mine[i])?;
                }
            }
        }
        Ok(())
    }

    fn fire(&mut self, queued: &Queued) -> Result<(), Halt> {
        let (rt, hooks) = (self.rt, self.hooks);
        if !self.within_limit(queued.index) {
            return Ok(());
        }
        let def = &rt.triggers()[queued.index];
        if let Some(limit) = def.limit {
            let set = Event::VarSet {
                key: limit_key(queued.index),
                value: self.stamp(limit),
            };
            self.state.apply(&set)?;
            self.events.push(set);
        }
        if let Some(name) = native_name(&def.effect) {
            let logic = rt.native(name);
            let logic = logic.ok_or_else(|| Halt::reject(format!("no native logic `{name}`")))?;
            return logic.run(self, hooks.env(), &queued.event);
        }
        hooks.fire(def, &queued.event, self)
    }

    /// Whether trigger `index` may still fire under its limit.
    fn within_limit(&self, index: usize) -> bool {
        match self.rt.triggers()[index].limit {
            None => true,
            Some(limit) => self.state.vars.get(&limit_key(index)) != Some(&self.stamp(limit)),
        }
    }

    /// The value that marks "already fired" for `limit` at the current moment.
    fn stamp(&self, limit: TriggerLimit) -> Value {
        let flow = &self.state.flow;
        match limit {
            TriggerLimit::OncePerTurn => Value::List(vec![
                Value::Int(flow.round.into()),
                Value::Int(flow.turn.into()),
            ]),
            TriggerLimit::OncePerRound => Value::Int(flow.round.into()),
            TriggerLimit::OncePerGame => Value::Bool(true),
        }
    }

    /// Asks `player` to order `labels`; returns indices into `labels`, first to last.
    fn order(
        &mut self,
        prompt: &str,
        player: PlayerId,
        labels: Vec<Sym>,
    ) -> Result<Vec<usize>, Halt> {
        let decision = Decision {
            prompt: Sym::new(prompt),
            kind: DecisionKind::Order(labels),
        };
        match self.choose(player, decision)? {
            Answer::Order(order) => Ok(order.into_iter().map(|i| i as usize).collect()),
            _ => Err(Halt::reject("answer does not fit the decision")),
        }
    }
}

fn limit_key(index: usize) -> VarKey {
    VarKey {
        module: ModuleId::new("engine"),
        name: Sym::new(&format!("trigger.{index}")),
    }
}

impl Ctx for Exec<'_> {
    fn state(&self) -> &GameState {
        &self.state
    }

    fn may_activate(&self, player: PlayerId, unit: UnitId) -> bool {
        let gate = self.rt.service::<dyn ActivationGate>();
        gate.is_none_or(|g| g.may_activate(&self.state, player, unit))
    }

    fn rng(&mut self) -> &mut CommandRng {
        &mut self.rng
    }

    fn emit(&mut self, e: Event) -> Result<(), Halt> {
        self.pipeline(e, &mut BTreeSet::new())
    }

    fn window(&mut self, id: &WindowId) -> Result<(), Halt> {
        self.open_window(id)
    }

    fn choose(&mut self, chooser: PlayerId, d: Decision) -> Result<Answer, Halt> {
        if self.cursor >= MAX_DECISIONS {
            return Err(Halt::reject("command asked too many decisions"));
        }
        let Some(answer) = self.tape.get(self.cursor) else {
            return Err(Halt::NeedsDecision {
                decision: d,
                chooser,
            });
        };
        if !answer_fits(&d, answer) {
            return Err(Halt::reject("recorded answer does not fit the decision"));
        }
        self.cursor += 1;
        Ok(answer.clone())
    }
}
