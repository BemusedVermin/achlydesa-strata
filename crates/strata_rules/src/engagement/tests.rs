//! The engagement memo: the same answers as the service, each asked once.

use std::sync::Mutex;

use strata_kernel::Space;
use strata_model::{FigureId, GameState};

use super::{Engagement, EngagementMemo};
use crate::engine::{Ctx, Halt};
use crate::native::{NativeEnv, NoEnv};
use crate::query::LogicError;

/// Enemies 1 and 2 stand on fixed hexes; a space is engaged with each enemy one step from it.
/// Leaving enemy 2 never provokes. Counts how often it is asked.
#[derive(Default)]
struct Counting {
    engaged: Mutex<usize>,
    provokes: Mutex<usize>,
}

const ENEMIES: [(FigureId, (i32, i32)); 2] = [(FigureId(1), (2, 0)), (FigureId(2), (0, 2))];

impl Engagement for Counting {
    fn engaged(
        &self,
        _: &GameState,
        _: &dyn NativeEnv,
        _: FigureId,
        at: Space,
    ) -> Result<Vec<FigureId>, LogicError> {
        *self.engaged.lock().unwrap() += 1;
        let near = |(q, r): (i32, i32)| Space::new(q, r, at.z).hex().distance(at.hex()) == 1;
        let found = ENEMIES.iter().filter(|(_, at)| near(*at));
        Ok(found.map(|(id, _)| *id).collect())
    }

    fn leaving_provokes(
        &self,
        _: &GameState,
        _: &dyn NativeEnv,
        _: FigureId,
        enemy: FigureId,
    ) -> Result<bool, LogicError> {
        *self.provokes.lock().unwrap() += 1;
        Ok(enemy != FigureId(2))
    }

    fn leave(
        &self,
        _: &mut dyn Ctx,
        _: &dyn NativeEnv,
        _: FigureId,
        _: &[Vec<Space>],
    ) -> Result<(), Halt> {
        Ok(())
    }
}

fn space((q, r): (i32, i32)) -> Space {
    Space::new(q, r, 1)
}

#[test]
fn the_memo_asks_each_space_and_enemy_once() {
    let service = Counting::default();
    let state = GameState::default();
    let memo = EngagementMemo::new(&service, &state, &NoEnv);
    let me = FigureId(0);
    let (near, far) = (space((1, 0)), space((-3, 0)));
    for _ in 0..3 {
        assert_eq!(memo.reprisals(me, &[near], &[far]).unwrap(), [FigureId(1)]);
    }
    assert_eq!(*service.engaged.lock().unwrap(), 2);
    assert_eq!(*service.provokes.lock().unwrap(), 1);
}

#[test]
fn a_walk_owes_each_enemy_once_in_the_order_it_is_left() {
    let service = Counting::default();
    let state = GameState::default();
    let memo = EngagementMemo::new(&service, &state, &NoEnv);
    // Next to enemy 1, away, back next to it, away again.
    let walk: Vec<Vec<Space>> = [(1, 0), (-1, 0), (1, 0), (-1, 0)]
        .map(|at| vec![space(at)])
        .to_vec();
    assert_eq!(memo.owed_along(FigureId(0), &walk).unwrap(), [FigureId(1)]);
    // Enemy 2 is left, but leaving it never provokes.
    let walk = vec![vec![space((0, 1))], vec![space((-3, 0))]];
    assert!(memo.owed_along(FigureId(0), &walk).unwrap().is_empty());
}

#[test]
fn the_memo_answers_like_the_service() {
    let service = Counting::default();
    let state = GameState::default();
    let memo = EngagementMemo::new(&service, &state, &NoEnv);
    let grid: Vec<Space> = (-2..3)
        .flat_map(|q| (-2..3).map(move |r| space((q, r))))
        .collect();
    for &from in &grid {
        for &to in &grid {
            let plain = service.reprisals(&state, &NoEnv, FigureId(0), &[from], &[to]);
            let kept = memo.reprisals(FigureId(0), &[from], &[to]);
            assert_eq!(plain.unwrap(), kept.unwrap(), "{from:?} to {to:?}");
        }
    }
}
