//! The engagement memo: the same answers as the service, each asked once.

use std::sync::Mutex;

use strata_kernel::Space;
use strata_model::{FigureId, GameState};

use super::{Engagement, EngagementMemo};
use crate::engine::{Ctx, Halt};
use crate::native::{NativeEnv, NoEnv};
use crate::query::LogicError;

/// Enemies 1 and 2 stand on fixed hexes. A space is engaged with each enemy one step from it, or
/// up to [`LONG_REACH`] steps for figure [`LONG`]. Leaving provokes enemy 1 but not enemy 2, and
/// the other way round for figure [`LONG`]. Counts how often it is asked.
#[derive(Default)]
struct Counting {
    engaged: Mutex<usize>,
    provokes: Mutex<usize>,
}

const ENEMIES: [(FigureId, (i32, i32)); 2] = [(FigureId(1), (2, 0)), (FigureId(2), (0, 2))];

/// The figure whose engagement and reprisals differ from everyone else's.
const LONG: FigureId = FigureId(3);

/// How many steps away [`LONG`] is engaged.
const LONG_REACH: u32 = 2;

/// How many steps away `figure` is engaged.
fn reach(figure: FigureId) -> u32 {
    if figure == LONG { LONG_REACH } else { 1 }
}

/// Whether leaving `enemy` provokes a reprisal against `figure`.
fn provokes(figure: FigureId, enemy: FigureId) -> bool {
    (enemy == FigureId(2)) == (figure == LONG)
}

impl Engagement for Counting {
    fn engaged(
        &self,
        _: &GameState,
        _: &dyn NativeEnv,
        figure: FigureId,
        at: Space,
    ) -> Result<Vec<FigureId>, LogicError> {
        *self.engaged.lock().unwrap() += 1;
        let near = |(q, r): (i32, i32)| {
            let steps = Space::new(q, r, at.z).hex().distance(at.hex());
            (1..=reach(figure)).contains(&steps)
        };
        let found = ENEMIES.iter().filter(|(_, at)| near(*at));
        Ok(found.map(|(id, _)| *id).collect())
    }

    fn leaving_provokes(
        &self,
        _: &GameState,
        _: &dyn NativeEnv,
        figure: FigureId,
        enemy: FigureId,
    ) -> Result<bool, LogicError> {
        *self.provokes.lock().unwrap() += 1;
        Ok(provokes(figure, enemy))
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

/// The rule written out by hand: the enemies within `figure`'s reach of some cell of `before`
/// and of no cell of `after`, whose leaving provokes, ordered by ID.
fn expected(figure: FigureId, before: &[Space], after: &[Space]) -> Vec<FigureId> {
    let near = |cells: &[Space], (q, r): (i32, i32)| {
        cells.iter().any(|cell| {
            let steps = cell.hex().distance(space((q, r)).hex());
            steps >= 1 && steps <= reach(figure)
        })
    };
    ENEMIES
        .iter()
        .filter(|&&(id, at)| near(before, at) && !near(after, at) && provokes(figure, id))
        .map(|&(id, _)| id)
        .collect()
}

#[test]
fn the_memo_answers_like_the_rule() {
    let service = Counting::default();
    let state = GameState::default();
    let memo = EngagementMemo::new(&service, &state, &NoEnv);
    let grid: Vec<Space> = (-2..4)
        .flat_map(|q| (-2..4).map(move |r| space((q, r))))
        .collect();
    // One-cell bodies, two-cell bodies in two directions, and a three-cell line.
    let mut bodies: Vec<Vec<Space>> = grid.iter().map(|&at| vec![at]).collect();
    for &at in &grid {
        let step = |dq: i32, dr: i32, n: i32| Space::new(at.q + dq * n, at.r + dr * n, at.z);
        bodies.push(vec![at, step(1, 0, 1)]);
        bodies.push(vec![at, step(0, 1, 1)]);
        bodies.push(vec![at, step(1, -1, 1), step(1, -1, 2)]);
    }
    let mut seen = [0usize; 2];
    for before in &bodies {
        for after in &bodies {
            // Both figures go through the same memo, so its keys must tell them apart.
            for (i, figure) in [FigureId(0), LONG].into_iter().enumerate() {
                let kept = memo.reprisals(figure, before, after).unwrap();
                let want = expected(figure, before, after);
                assert_eq!(kept, want, "{figure:?}: {before:?} to {after:?}");
                seen[i] += want.len();
            }
        }
    }
    // The figures answer differently, and both owe reprisals somewhere.
    assert!(seen.iter().all(|&n| n > 0), "{seen:?}");
    assert_eq!(
        memo.reprisals(FigureId(0), &[space((1, 0))], &[space((-2, 0))])
            .unwrap(),
        [FigureId(1)]
    );
    // Figure 3 also reaches enemy 2 from (1, 0), and only leaving enemy 2 provokes it.
    assert_eq!(
        memo.reprisals(LONG, &[space((1, 0))], &[space((-2, 0))])
            .unwrap(),
        [FigureId(2)]
    );
}
