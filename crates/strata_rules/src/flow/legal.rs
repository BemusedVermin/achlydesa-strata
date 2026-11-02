//! Legal commands, derived from the phase, the activation and the actions (design §12.4).

use strata_kernel::{DefId, Sym, Value};
use strata_model::{Command, FigureStatus, GameState, PlayerId, UnitId};

use super::action::ActionRunner;
use super::activation::ActivationRules;
use super::tree::PhaseTree;
use crate::runtime::RulesRuntime;

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
    /// What `player` may submit now: nothing while a decision is pending or out of turn.
    pub(crate) fn legal_commands(
        &self,
        rt: &RulesRuntime,
        s: &GameState,
        player: PlayerId,
    ) -> Vec<Command> {
        if s.pending.is_some() || s.flow.active != player {
            return Vec::new();
        }
        match &s.flow.activation {
            None => self.activations(s, player),
            Some(act) => {
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
                            Some(_) => self.runner.targets(def, figure, s).unwrap_or_default(),
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
    fn activations(&self, s: &GameState, player: PlayerId) -> Vec<Command> {
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
            .map(|(i, _)| Command::ActivateUnit(UnitId(i as u32)))
            .collect()
    }
}
