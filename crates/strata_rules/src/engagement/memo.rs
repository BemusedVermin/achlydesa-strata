//! Engagement answers kept for one unchanged state (design §7.4).
//!
//! A movement query judges many steps, and every step asks whom each covered space is engaged
//! with. Those answers depend only on the state, the figure and the space, so one query asks the
//! service each of them once.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use strata_kernel::Space;
use strata_model::{FigureId, GameState};

use super::Engagement;
use crate::native::NativeEnv;
use crate::query::LogicError;

/// An [`Engagement`] service's answers about one state, each asked of it at most once.
///
/// Errors are not kept, so a failing vote fails every question that needs it.
pub struct EngagementMemo<'a, E: ?Sized> {
    service: &'a E,
    state: &'a GameState,
    env: &'a dyn NativeEnv,
    engaged: RefCell<BTreeMap<(FigureId, Space), Vec<FigureId>>>,
    provokes: RefCell<BTreeMap<(FigureId, FigureId), bool>>,
}

impl<'a, E: Engagement + ?Sized> EngagementMemo<'a, E> {
    /// An empty memo of `service`'s answers about `state`.
    pub fn new(service: &'a E, state: &'a GameState, env: &'a dyn NativeEnv) -> Self {
        Self {
            service,
            state,
            env,
            engaged: RefCell::default(),
            provokes: RefCell::default(),
        }
    }

    /// [`Engagement::engaged`], asked once per figure and space.
    pub fn engaged(&self, figure: FigureId, at: Space) -> Result<Vec<FigureId>, LogicError> {
        if let Some(hit) = self.engaged.borrow().get(&(figure, at)) {
            return Ok(hit.clone());
        }
        let found = self.service.engaged(self.state, self.env, figure, at)?;
        self.engaged
            .borrow_mut()
            .insert((figure, at), found.clone());
        Ok(found)
    }

    /// [`Engagement::leaving_provokes`], asked once per figure and enemy.
    pub fn leaving_provokes(&self, figure: FigureId, enemy: FigureId) -> Result<bool, LogicError> {
        if let Some(&hit) = self.provokes.borrow().get(&(figure, enemy)) {
            return Ok(hit);
        }
        let owed = (self.service).leaving_provokes(self.state, self.env, figure, enemy)?;
        self.provokes.borrow_mut().insert((figure, enemy), owed);
        Ok(owed)
    }

    /// The enemies engaged with any of the spaces a body covers, ordered by ID.
    fn engaged_body(
        &self,
        figure: FigureId,
        cells: &[Space],
    ) -> Result<BTreeSet<FigureId>, LogicError> {
        let mut found = BTreeSet::new();
        for &cell in cells {
            found.extend(self.engaged(figure, cell)?);
        }
        Ok(found)
    }

    /// The enemies owed a reprisal when `figure`, covering the spaces `before`, steps to cover
    /// `after`, ordered by ID. The body is judged as a whole: an enemy is left only if some space
    /// of `before` is engaged with it, no space of `after` is, and leaving it provokes. A one-cell
    /// figure passes one space on each side.
    pub fn reprisals(
        &self,
        figure: FigureId,
        before: &[Space],
        after: &[Space],
    ) -> Result<Vec<FigureId>, LogicError> {
        let kept = self.engaged_body(figure, after)?;
        let mut owed = Vec::new();
        for enemy in self.engaged_body(figure, before)? {
            if !kept.contains(&enemy) && self.leaving_provokes(figure, enemy)? {
                owed.push(enemy);
            }
        }
        Ok(owed)
    }

    /// The enemies owed a reprisal along `walk`, the bodies a figure covers where it stands and
    /// after each step, in the order the walk first leaves them. An enemy left twice is owed one
    /// reprisal.
    pub fn owed_along(
        &self,
        figure: FigureId,
        walk: &[Vec<Space>],
    ) -> Result<Vec<FigureId>, LogicError> {
        let mut owed: Vec<FigureId> = Vec::new();
        for step in walk.windows(2) {
            for enemy in self.reprisals(figure, &step[0], &step[1])? {
                if !owed.contains(&enemy) {
                    owed.push(enemy);
                }
            }
        }
        Ok(owed)
    }
}
