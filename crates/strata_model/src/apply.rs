//! `GameState::apply`, the only mutator of game state (design §5.1).

use strata_kernel::{DefId, Value};
use thiserror::Error;

use crate::event::{Event, Visibility};
use crate::ids::{FigureId, PlayerId, RelicId, UnitId};
use crate::state::{
    FigureState, FigureStatus, GameState, Marker, PhaseFrame, PlayerState, Token, UnitState,
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
    /// Another figure already stands on the target space.
    #[error("space is occupied by figure {0:?}")]
    Occupied(FigureId),
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
            } => {
                let fig = self.live_figure(*figure)?;
                let from = fig.location;
                if let Some(&other) = self.board_changes.occupancy.get(to) {
                    if other != *figure {
                        return Err(ApplyError::Occupied(other));
                    }
                }
                if let Some(from) = from {
                    self.board_changes.occupancy.remove(&from);
                }
                self.board_changes.occupancy.insert(*to, *figure);
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
            Event::FigureDestroyed(id) => {
                let at = self.live_figure(*id)?.location;
                if let Some(at) = at {
                    self.board_changes.occupancy.remove(&at);
                }
                let fig = &mut self.figures[id.index()];
                fig.location = None;
                fig.status = FigureStatus::Destroyed;
            }
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
                r.location = None;
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
            Event::DiceRolled { .. } | Event::Custom { .. } => {}
        }
        Ok(())
    }

    fn figure(&self, id: FigureId) -> Result<&FigureState, ApplyError> {
        self.figures.get(id.index()).ok_or(ApplyError::NoFigure(id))
    }

    /// A figure that is still in play: in reserve or on the board.
    fn live_figure(&self, id: FigureId) -> Result<&FigureState, ApplyError> {
        let fig = self.figure(id)?;
        match fig.status {
            FigureStatus::Reserve | FigureStatus::OnBoard => Ok(fig),
            status => Err(ApplyError::WrongStatus(id, status)),
        }
    }

    fn unit(&self, id: UnitId) -> Result<&UnitState, ApplyError> {
        self.units.get(id.index()).ok_or(ApplyError::NoUnit(id))
    }

    fn player(&self, id: PlayerId) -> Result<&PlayerState, ApplyError> {
        self.players.get(id.index()).ok_or(ApplyError::NoPlayer(id))
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
