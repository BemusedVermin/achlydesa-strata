//! `GameState::apply`, the only mutator of game state (design §5.1).

use strata_kernel::{DefId, Space, Value};
use thiserror::Error;

use crate::ability_key::AbilityKey;
use crate::event::{Event, Visibility};
use crate::ids::{FigureId, PlayerId, RelicId, UnitId};
use crate::state::{
    AbilityInstance, AbilityOwner, Activation, FigureState, FigureStatus, GameState, Marker,
    PhaseFrame, PlayerState, Token, UnitState,
};

/// An event that cannot be applied to the state it was given. The state is unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ApplyError {
    /// The figure ID is out of range.
    #[error("no figure {0:?}")]
    NoFigure(FigureId),
    /// The unit ID is out of range.
    #[error("no unit {0:?}")]
    NoUnit(UnitId),
    /// The player ID is out of range.
    #[error("no player {0:?}")]
    NoPlayer(PlayerId),
    /// The relic ID is out of range.
    #[error("no relic {0:?}")]
    NoRelic(RelicId),
    /// The figure's status does not allow the event.
    #[error("figure {0:?} cannot take this event while {1:?}")]
    WrongStatus(FigureId, FigureStatus),
    /// Another figure already covers one of the target spaces.
    #[error("space is occupied by figure {0:?}")]
    Occupied(FigureId),
    /// A move's `cells` do not include its anchor; an empty list never does.
    #[error("figure {0:?}'s cells do not include its anchor")]
    AnchorNotCovered(FigureId),
    /// A move would change how many spaces a figure on the board covers, as an emitter that
    /// left out some of its cells would.
    #[error("figure {0:?} covers {1} spaces, but its move lists {2}")]
    BodyResized(FigureId, usize, usize),
    /// A marker change of zero, or a count that would leave `1..=u32::MAX` (or `0` on removal).
    #[error("marker count for figure {0:?} would be out of range")]
    MarkerCount(FigureId),
    /// The token kind is not in the owner's hand.
    #[error("token {0:?} is not in the owner's hand")]
    TokenNotInHand(DefId),
    /// The unit has no hidden token at that index.
    #[error("unit {0:?} has no hidden token at index {1}")]
    NoHiddenToken(UnitId, u8),
    /// The phase being exited is not the top of the stack.
    #[error("phase {0:?} is not the top phase")]
    PhaseMismatch(DefId),
    /// An activation is already under way.
    #[error("an activation is already under way")]
    ActivationBusy,
    /// No activation is under way.
    #[error("no activation is under way")]
    NoActivation,
    /// The figure has no slot budget in the activation.
    #[error("figure {0:?} has no slots in this activation")]
    NoSlots(FigureId),
    /// The relic is already face up.
    #[error("relic {0:?} is already face up")]
    RelicFaceUp(RelicId),
    /// No attached ability instance matches.
    #[error("no attached ability {0:?}")]
    NoAbility(AbilityKey),
    /// The relic has no holder to release.
    #[error("relic {0:?} is not held")]
    RelicNotHeld(RelicId),
    /// The relic already has a holder.
    #[error("relic {0:?} is already held")]
    RelicHeld(RelicId),
}

impl GameState {
    /// Applies one event. On error the state is left unchanged.
    pub fn apply(&mut self, event: &Event) -> Result<(), ApplyError> {
        match event {
            Event::FigureMoved {
                figure,
                to,
                rotation,
                cells,
                ..
            } => {
                self.live_figure(*figure)?;
                self.cover(*figure, *to, cells)?;
                let fig = &mut self.figures[figure.index()];
                fig.location = Some(*to);
                fig.rotation = *rotation;
                fig.status = FigureStatus::OnBoard;
            }
            Event::WoundsApplied {
                figure,
                wound,
                amount,
            } => self.change_marker(*figure, *wound, i64::from(*amount), None, None)?,
            Event::MarkerChanged {
                figure,
                marker,
                delta,
                source,
                expiry,
            } => self.change_marker(*figure, *marker, i64::from(*delta), *source, *expiry)?,
            Event::FigureDestroyed(id) => self.take_off(*id, FigureStatus::Destroyed)?,
            Event::FigureRemoved(id) => self.take_off(*id, FigureStatus::Removed)?,
            Event::TokenPlaced { unit, token } => self.place_token(*unit, token)?,
            Event::TokenRevealed { unit, index } => {
                let hidden = self.unit(*unit)?.tokens.get(usize::from(*index));
                if hidden.is_none_or(|t| t.visibility != Visibility::Hidden) {
                    return Err(ApplyError::NoHiddenToken(*unit, *index));
                }
                self.units[unit.index()].tokens[usize::from(*index)].visibility =
                    Visibility::Public;
            }
            Event::PhaseEntered(phase) => self.flow.phases.push(PhaseFrame {
                phase: *phase,
                data: Default::default(),
            }),
            Event::PhaseExited(phase) => match self.flow.phases.last() {
                Some(top) if top.phase == *phase => {
                    self.flow.phases.pop();
                }
                _ => return Err(ApplyError::PhaseMismatch(*phase)),
            },
            Event::ActivationStarted { unit, step, slots } => {
                self.unit(*unit)?;
                if self.flow.activation.is_some() {
                    return Err(ApplyError::ActivationBusy);
                }
                self.flow.activation = Some(Activation {
                    unit: *unit,
                    step: step.clone(),
                    slots: slots.clone(),
                });
            }
            Event::SlotsChanged {
                figure,
                slot,
                remaining,
            } => {
                let act = self
                    .flow
                    .activation
                    .as_mut()
                    .ok_or(ApplyError::NoActivation)?;
                let slots = act
                    .slots
                    .get_mut(figure)
                    .ok_or(ApplyError::NoSlots(*figure))?;
                slots.insert(slot.clone(), *remaining);
            }
            Event::StepChanged(step) => {
                let act = self
                    .flow
                    .activation
                    .as_mut()
                    .ok_or(ApplyError::NoActivation)?;
                act.step = step.clone();
            }
            Event::ActivationEnded => {
                if self.flow.activation.take().is_none() {
                    return Err(ApplyError::NoActivation);
                }
            }
            Event::RelicClaimed { relic, by } => {
                self.player(*by)?;
                let r = self
                    .relics
                    .get_mut(relic.index())
                    .ok_or(ApplyError::NoRelic(*relic))?;
                if r.holder.is_some() {
                    return Err(ApplyError::RelicHeld(*relic));
                }
                r.holder = Some(*by);
            }
            Event::RelicReleased(relic) => {
                let r = self.relics.get_mut(relic.index());
                let r = r.ok_or(ApplyError::NoRelic(*relic))?;
                r.holder.take().ok_or(ApplyError::RelicNotHeld(*relic))?;
            }
            Event::RelicRevealed(relic) => {
                let r = self.relics.get_mut(relic.index());
                let r = r.ok_or(ApplyError::NoRelic(*relic))?;
                if !r.face_down {
                    return Err(ApplyError::RelicFaceUp(*relic));
                }
                r.face_down = false;
            }
            Event::AbilityAttached { def, owner, params } => {
                self.check_owner(*owner)?;
                self.abilities.push(AbilityInstance {
                    def: *def,
                    owner: *owner,
                    params: params.clone(),
                });
            }
            Event::AbilityDetached { def, owner } => {
                let at = self
                    .abilities
                    .iter()
                    .position(|a| a.def == *def && a.owner == *owner);
                self.abilities
                    .remove(at.ok_or(ApplyError::NoAbility(*def))?);
            }
            Event::VarSet { key, value } => {
                if *value == Value::Nil {
                    self.vars.remove(key);
                } else {
                    self.vars.insert(key.clone(), value.clone());
                }
            }
            Event::ControlChanged { figure, controller } => {
                self.player(*controller)?;
                self.figure(*figure)?;
                self.figures[figure.index()].controller = *controller;
            }
            Event::TokensDealt { player, tokens } => {
                self.player(*player)?;
                self.players[player.index()].hand.clone_from(tokens);
            }
            Event::TokensDiscarded(unit) => {
                self.unit(*unit)?;
                self.units[unit.index()].tokens.clear();
            }
            Event::RoundStarted(round) => {
                self.flow.round = *round;
                self.flow.turn = 0;
            }
            Event::TurnStarted { turn, active } => {
                self.player(*active)?;
                self.flow.turn = *turn;
                self.flow.active = *active;
            }
            Event::DiceRolled { .. } | Event::Custom { .. } => {}
        }
        Ok(())
    }

    fn figure(&self, id: FigureId) -> Result<&FigureState, ApplyError> {
        self.figures.get(id.index()).ok_or(ApplyError::NoFigure(id))
    }

    /// A figure that is still in play: in reserve or on the board.
    /// Takes a figure in play off the board for good, leaving it with `status`.
    fn take_off(&mut self, id: FigureId, status: FigureStatus) -> Result<(), ApplyError> {
        self.live_figure(id)?;
        self.board_changes.occupancy.retain(|_, f| *f != id);
        let fig = &mut self.figures[id.index()];
        fig.location = None;
        fig.status = status;
        Ok(())
    }

    fn live_figure(&self, id: FigureId) -> Result<&FigureState, ApplyError> {
        let fig = self.figure(id)?;
        match fig.status {
            FigureStatus::Reserve | FigureStatus::OnBoard => Ok(fig),
            status => Err(ApplyError::WrongStatus(id, status)),
        }
    }

    /// Moves `figure`'s entries in the occupancy index to `body`, the spaces it covers with its
    /// anchor on `to`. Refuses a body without its anchor, one of another size than the body the
    /// figure already covers, or one that overlaps another figure.
    fn cover(&mut self, figure: FigureId, to: Space, body: &[Space]) -> Result<(), ApplyError> {
        if !body.contains(&to) {
            return Err(ApplyError::AnchorNotCovered(figure));
        }
        let held = self.covered(figure).len();
        if held != 0 && held != body.len() {
            return Err(ApplyError::BodyResized(figure, held, body.len()));
        }
        let mut others = body.iter().filter_map(|s| self.occupant(*s));
        if let Some(other) = others.find(|o| *o != figure) {
            return Err(ApplyError::Occupied(other));
        }
        let index = &mut self.board_changes.occupancy;
        index.retain(|_, f| *f != figure);
        index.extend(body.iter().map(|s| (*s, figure)));
        Ok(())
    }

    fn unit(&self, id: UnitId) -> Result<&UnitState, ApplyError> {
        self.units.get(id.index()).ok_or(ApplyError::NoUnit(id))
    }

    fn player(&self, id: PlayerId) -> Result<&PlayerState, ApplyError> {
        self.players.get(id.index()).ok_or(ApplyError::NoPlayer(id))
    }

    fn check_owner(&self, owner: AbilityOwner) -> Result<(), ApplyError> {
        match owner {
            AbilityOwner::Unit(u) => self.unit(u).map(drop),
            AbilityOwner::Player(p) => self.player(p).map(drop),
            AbilityOwner::Relic(r) => match self.relics.get(r.index()) {
                Some(_) => Ok(()),
                None => Err(ApplyError::NoRelic(r)),
            },
        }
    }

    fn place_token(&mut self, unit: UnitId, token: &Token) -> Result<(), ApplyError> {
        let owner = self.unit(unit)?.owner;
        self.player(owner)?;
        let hand = &mut self.players[owner.index()].hand;
        let at = hand
            .iter()
            .position(|kind| *kind == token.kind)
            .ok_or(ApplyError::TokenNotInHand(token.kind))?;
        hand.remove(at);
        self.units[unit.index()].tokens.push(token.clone());
        Ok(())
    }

    /// Adds `delta` to the marker matching `(def, source, expiry)`, dropping it at zero.
    fn change_marker(
        &mut self,
        id: FigureId,
        def: DefId,
        delta: i64,
        source: Option<FigureId>,
        expiry: Option<u32>,
    ) -> Result<(), ApplyError> {
        let markers = &self.figure(id)?.markers;
        let at = markers
            .iter()
            .position(|m| m.def == def && m.source == source && m.expiry == expiry);
        let current = at.map_or(0, |i| i64::from(markers[i].count));
        let count = u32::try_from(current + delta)
            .ok()
            .filter(|_| delta != 0)
            .ok_or(ApplyError::MarkerCount(id))?;
        let markers = &mut self.figures[id.index()].markers;
        match (at, count) {
            (Some(i), 0) => {
                markers.remove(i);
            }
            (Some(i), n) => markers[i].count = n,
            (None, 0) => {}
            (None, n) => markers.push(Marker {
                def,
                count: n,
                source,
                expiry,
            }),
        }
        Ok(())
    }
}
