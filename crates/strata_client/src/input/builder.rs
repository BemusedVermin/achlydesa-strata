//! Command building from the legal commands: select a unit, a figure, an action, then a target.
//!
//! The builder never invents a command. Each one it returns is an element of the legal list the
//! rules produced, so the UI cannot submit anything the rules would not offer.

use strata_kernel::{DefId, Space, Value};
use strata_model::{Command, FigureId, GameState, PlayerId, UnitId};

use super::decision::Highlights;
use super::pick::{Pick, Refusal};

/// An action's argument, as the board or a dialog shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A figure, chosen by clicking it.
    Figure(FigureId),
    /// A space, chosen by clicking it. Written `[q, r, z]` or `{ q, r, z }`.
    Space(Space),
    /// Anything else, chosen from a list.
    Other(Value),
}

impl Target {
    /// Reads `args`: an integer naming a figure of `state` is a figure, three coordinates are a
    /// space.
    pub fn of(args: &Value, state: &GameState) -> Self {
        let int = |v: Option<&Value>| match v {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        };
        let space = |q, r, z| match (int(q), int(r), int(z)) {
            (Some(q), Some(r), Some(z)) => Some(Space::new(
                i32::try_from(q).ok()?,
                i32::try_from(r).ok()?,
                u32::try_from(z).ok()?,
            )),
            _ => None,
        };
        let found = match args {
            Value::Int(n) => usize::try_from(*n)
                .ok()
                .filter(|i| *i < state.figures.len())
                .map(|i| Self::Figure(FigureId(i as u32))),
            Value::List(xs) if xs.len() == 3 => {
                space(xs.first(), xs.get(1), xs.get(2)).map(Self::Space)
            }
            Value::Map(m) if m.len() == 3 => {
                let get = |k: &str| m.get(&strata_kernel::Sym::new(k));
                space(get("q"), get("r"), get("z")).map(Self::Space)
            }
            _ => None,
        };
        found.unwrap_or_else(|| Self::Other(args.clone()))
    }

    fn matches(&self, pick: &Pick) -> bool {
        match self {
            Self::Figure(f) => pick.figure == Some(*f),
            Self::Space(s) => pick.space == Some(*s),
            Self::Other(_) => false,
        }
    }
}

/// Where command building stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing is legal for the viewer.
    Idle,
    /// Pick a unit to activate.
    Unit,
    /// Pick a figure of the activated unit.
    Figure,
    /// Pick an action for the selected figure.
    Action(FigureId),
    /// Pick a target for the selected action.
    Target(FigureId, DefId),
    /// No figure can act; the activation can only end.
    End,
}

/// The selection so far, over one list of legal commands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandBuilder {
    viewer: PlayerId,
    legal: Vec<Command>,
    figure: Option<FigureId>,
    action: Option<DefId>,
}

impl CommandBuilder {
    /// A builder for `viewer` over `legal`, the commands the rules accept now.
    pub fn new(viewer: PlayerId, legal: Vec<Command>) -> Self {
        Self {
            viewer,
            legal,
            figure: None,
            action: None,
        }
    }

    /// The current step.
    pub fn step(&self) -> Step {
        match (self.figure, self.action) {
            (Some(f), Some(a)) => Step::Target(f, a),
            (Some(f), None) => Step::Action(f),
            _ if !self.actors().is_empty() => Step::Figure,
            _ if !self.units().is_empty() => Step::Unit,
            _ if self.can_end() => Step::End,
            _ => Step::Idle,
        }
    }

    /// Units the viewer may activate.
    pub fn units(&self) -> Vec<UnitId> {
        let units = self.legal.iter().filter_map(|c| match c {
            Command::ActivateUnit(u) => Some(*u),
            _ => None,
        });
        dedup(units)
    }

    /// Figures that have an action they may take.
    pub fn actors(&self) -> Vec<FigureId> {
        dedup(self.actions_of(|_| true).map(|(f, _, _)| f))
    }

    /// The actions the selected figure may take.
    pub fn actions(&self) -> Vec<DefId> {
        dedup(self.chosen_figure().map(|(_, a, _)| a))
    }

    /// The legal commands for the selected figure and action, one per target.
    pub fn options(&self) -> Vec<&Command> {
        let action = self.action;
        let picked = self.legal.iter().filter(|c| match c {
            Command::Action {
                figure, action: a, ..
            } => Some(*figure) == self.figure && Some(*a) == action,
            _ => false,
        });
        picked.collect()
    }

    /// Whether the viewer may end the current activation.
    pub fn can_end(&self) -> bool {
        self.legal.contains(&Command::EndActivation)
    }

    /// Picks `unit` to activate, as a button does instead of a click on one of its figures.
    pub fn choose_unit(&self, unit: UnitId) -> Option<Command> {
        let command = Command::ActivateUnit(unit);
        self.legal.contains(&command).then_some(command)
    }

    /// Steps back: drops the action, then the figure.
    pub fn back(&mut self) {
        if self.action.take().is_none() {
            self.figure = None;
        }
    }

    /// Picks an action for the selected figure. An action without targets is ready at once.
    pub fn choose_action(&mut self, action: DefId) -> Option<Command> {
        if self.figure.is_none() || !self.actions().contains(&action) {
            return None;
        }
        self.action = Some(action);
        let options = self.options();
        match options.as_slice() {
            [
                only @ Command::Action {
                    args: Value::Nil, ..
                },
            ] => {
                let command = (*only).clone();
                self.reset();
                Some(command)
            }
            _ => None,
        }
    }

    /// Takes the `index`th of [`options`](Self::options), for targets chosen from a list.
    pub fn choose_option(&mut self, index: usize) -> Option<Command> {
        let command = self.options().get(index).map(|c| (*c).clone())?;
        self.reset();
        Some(command)
    }

    /// Applies a click: it may finish a command, move the selection on, or be refused.
    pub fn click(&mut self, state: &GameState, pick: &Pick) -> Result<Option<Command>, Refusal> {
        match self.step() {
            Step::Idle => Err(Refusal::NothingToDo),
            Step::Unit => {
                let figure = pick.figure.ok_or(Refusal::NoFigure)?;
                let unit = state.figures.get(figure.index()).map(|f| f.unit);
                match unit {
                    Some(u) if self.units().contains(&u) => Ok(Some(Command::ActivateUnit(u))),
                    _ => Err(self.why_not(state, figure, Refusal::CannotActivate)),
                }
            }
            Step::End => {
                let figure = pick.figure.ok_or(Refusal::NoFigure)?;
                Err(self.why_not(state, figure, Refusal::CannotAct))
            }
            step => {
                if matches!(step, Step::Target(..)) {
                    let hit = self.options().into_iter().find(|c| match c {
                        Command::Action { args, .. } => Target::of(args, state).matches(pick),
                        _ => false,
                    });
                    if let Some(command) = hit.cloned() {
                        self.reset();
                        return Ok(Some(command));
                    }
                }
                let figure = pick.figure;
                match figure {
                    Some(f) if self.actors().contains(&f) => {
                        self.figure = Some(f);
                        self.action = None;
                        Ok(None)
                    }
                    _ if matches!(step, Step::Target(..)) => Err(Refusal::NotATarget),
                    Some(f) => Err(self.why_not(state, f, Refusal::CannotAct)),
                    None => Err(Refusal::NoFigure),
                }
            }
        }
    }

    /// What the board marks at this step.
    pub fn highlights(&self, state: &GameState) -> Highlights {
        let mut out = Highlights::default();
        match self.step() {
            Step::Idle | Step::End => {}
            Step::Unit => {
                let units = self.units();
                let figures = state.figures.iter().enumerate();
                let ready =
                    figures.filter(|(_, f)| f.location.is_some() && units.contains(&f.unit));
                out.figures = ready.map(|(i, _)| FigureId(i as u32)).collect();
            }
            Step::Figure => out.figures = self.actors().into_iter().collect(),
            Step::Action(figure) => {
                out.figures.insert(figure);
            }
            Step::Target(..) => {
                for command in self.options() {
                    if let Command::Action { args, .. } = command {
                        match Target::of(args, state) {
                            Target::Figure(f) => {
                                out.figures.insert(f);
                            }
                            Target::Space(s) => {
                                out.spaces.insert(s);
                            }
                            Target::Other(_) => {}
                        }
                    }
                }
            }
        }
        out
    }

    fn reset(&mut self) {
        self.figure = None;
        self.action = None;
    }

    fn why_not(&self, state: &GameState, figure: FigureId, otherwise: Refusal) -> Refusal {
        match state.figures.get(figure.index()) {
            Some(f) if f.controller != self.viewer => Refusal::NotYours,
            _ => otherwise,
        }
    }

    fn actions_of(
        &self,
        keep: impl Fn(FigureId) -> bool,
    ) -> impl Iterator<Item = (FigureId, DefId, &Value)> {
        self.legal.iter().filter_map(move |c| match c {
            Command::Action {
                figure,
                action,
                args,
            } if keep(*figure) => Some((*figure, *action, args)),
            _ => None,
        })
    }

    fn chosen_figure(&self) -> impl Iterator<Item = (FigureId, DefId, &Value)> {
        let chosen = self.figure;
        self.actions_of(move |f| Some(f) == chosen)
    }
}

/// `items` without repeats, in first-seen order.
fn dedup<T: PartialEq>(items: impl Iterator<Item = T>) -> Vec<T> {
    let mut out = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

#[cfg(test)]
mod tests;
