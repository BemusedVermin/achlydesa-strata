//! Legal commands, derived from the phase, the activation and the actions (design §12.4).

use strata_kernel::{DefId, Sym, Value};
use strata_model::{Command, FigureStatus, GameState, PlayerId, UnitId};

use super::action::ActionRunner;
use super::activation::ActivationRules;
use super::tree::PhaseTree;
use crate::native::{NativeActions, NativeEnv};
use crate::runtime::RulesRuntime;

/// Decides which units a player may activate now. A module provides it as a service; without one,
/// players may activate any of their units.
pub trait ActivationGate: Send + Sync {
    /// Whether `player` may activate `unit` in the current turn.
    fn may_activate(&self, state: &GameState, player: PlayerId, unit: UnitId) -> bool;
}

/// Everything `legal_commands` needs beyond the registrations.
pub struct Flow<'a> {
    /// The phase tree.
    pub tree: PhaseTree,
    /// The activation module's configuration.
    pub rules: ActivationRules,
    /// Evaluates action targeting.
    pub runner: &'a dyn ActionRunner,
    /// The phase in which players activate units.
    pub turn_phase: Sym,
}

impl Flow<'_> {
    /// What `player` may submit now: nothing while a decision is pending or out of turn. Native
    /// actions answer their targets with `env`, the hooks' environment that also resolves them.
    pub(crate) fn legal_commands(
        &self,
        rt: &RulesRuntime,
        env: &dyn NativeEnv,
        s: &GameState,
        player: PlayerId,
    ) -> Vec<Command> {
        if s.pending.is_some() || s.flow.active != player {
            return Vec::new();
        }
        match &s.flow.activation {
            None => self.activations(rt, s, player),
            Some(act) => {
                let runner = NativeActions::new(rt, Some(self.runner), env);
                let mut out = Vec::new();
                for &figure in &s.units[act.unit.index()].figures {
                    let fig = &s.figures[figure.index()];
                    if fig.controller != player || fig.status != FigureStatus::OnBoard {
                        continue;
                    }
                    for (i, def) in rt.actions().iter().enumerate() {
                        if !self.rules.can_pay(act, figure, &def.slots) {
                            continue;
                        }
                        let args = match def.targets {
                            Some(_) => runner.targets(def, figure, s).unwrap_or_default(),
                            None => vec![Value::Nil],
                        };
                        out.extend(args.into_iter().map(|args| Command::Action {
                            figure,
                            action: DefId(i as u32),
                            args,
                        }));
                    }
                }
                out.push(Command::EndActivation);
                out
            }
        }
    }

    /// Units `player` may activate, which is only possible inside the turn phase.
    fn activations(&self, rt: &RulesRuntime, s: &GameState, player: PlayerId) -> Vec<Command> {
        let gate = rt.service::<dyn ActivationGate>();
        let top = s.flow.phases.last().and_then(|f| self.tree.name(f.phase));
        if top != Some(&self.turn_phase) {
            return Vec::new();
        }
        let on_board = |f: &strata_model::FigureId| {
            s.figures
                .get(f.index())
                .is_some_and(|f| f.status == FigureStatus::OnBoard)
        };
        let units = s.units.iter().enumerate();
        units
            .filter(|(_, u)| u.owner == player && u.figures.iter().any(on_board))
            .filter(|(i, _)| gate.is_none_or(|g| g.may_activate(s, player, UnitId(*i as u32))))
            .map(|(i, _)| Command::ActivateUnit(UnitId(i as u32)))
            .collect()
    }
}
