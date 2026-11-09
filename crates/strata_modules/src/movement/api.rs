//! `ctx.movement`: `reachable(figure)` lists where a figure may stop, `move(figure, q, r, z, o)`
//! walks it there.
//!
//! A figure with a one-cell footprint is listed as `{ q, r, z }` and moved without `o`, since it
//! keeps its rotation. A larger footprint can reach a space in several rotations, so it is listed
//! as `{ q, r, z, o }`; `move` without `o` takes the cheapest rotation, the lowest on a tie. A
//! move that ends in a fall emits `figure_fell` with `{ figure, levels }`, the levels dropped.
//!
//! A move first resolves the reprisals for leaving engagement along its route, and does not move
//! a figure they take out of play. `preview(figure)` lists the same stops as `reachable`, as
//! `{ q, r, z, o, cost, path, stops_here, falls, provokes_reprisal }` tables, `o` only for larger
//! footprints. `path` lists the spaces the anchor steps through, as `{ q, r, z }` tables ending on
//! the stop, the same route `move` would walk.

use strata_kernel::{Rotation, Space, Sym, Value, ValueError};
use strata_model::{Event, FigureId};
use strata_rules::{ApiHandler, Ctx, Halt, NativeEnv, Reach};

/// The custom event emitted when a move ends in a fall.
pub const FIGURE_FELL: &str = "figure_fell";

/// Runs the functions of the `movement` namespace.
pub struct Api;

/// A figure ID from a script argument.
fn figure(args: &[Value]) -> Result<FigureId, Halt> {
    let first = args
        .first()
        .ok_or_else(|| Halt::reject("movement needs a figure"))?;
    let id = first.as_int().map_err(|e| Halt::reject(e.to_string()))?;
    u32::try_from(id)
        .map(FigureId)
        .map_err(|_| Halt::reject("movement: the figure is out of range"))
}

/// Reads `q, r, z` from the arguments after the figure.
fn space(args: &[Value]) -> Result<Space, Halt> {
    let bad = |e: ValueError| Halt::reject(format!("move: {e}"));
    let at = |i: usize| {
        args.get(i)
            .ok_or_else(|| Halt::reject("move needs q, r and z"))
    };
    let coord = |i: usize| -> Result<i32, Halt> {
        let n = at(i)?.as_int().map_err(bad)?;
        i32::try_from(n).map_err(|_| Halt::reject("move: a coordinate is out of range"))
    };
    let z = u32::try_from(coord(3)?).map_err(|_| Halt::reject("move: z must not be negative"))?;
    Ok(Space::new(coord(1)?, coord(2)?, z))
}

/// The rotation named by the optional fifth argument.
fn rotation(args: &[Value]) -> Result<Option<Rotation>, Halt> {
    let Some(given) = args.get(4) else {
        return Ok(None);
    };
    let steps = given
        .as_int()
        .map_err(|e| Halt::reject(format!("move: {e}")))?;
    let steps = u8::try_from(steps).ok().and_then(Rotation::new);
    steps
        .map(Some)
        .ok_or_else(|| Halt::reject("move: a rotation is 0 to 5"))
}

/// A reachable stop as scripts see it.
fn listed(reach: &Reach) -> Value {
    let s = reach.space;
    let mut at = vec![s.q, s.r, s.z as i32];
    if reach.cells.len() > 1 {
        at.push(i32::from(reach.rotation.steps()));
    }
    Value::List(at.into_iter().map(|n| Value::Int(i64::from(n))).collect())
}

/// A space as a `{ q, r, z }` table.
fn space_value(s: Space) -> Value {
    let fields = [
        ("q", i64::from(s.q)),
        ("r", i64::from(s.r)),
        ("z", i64::from(s.z)),
    ];
    let fields = fields
        .into_iter()
        .map(|(k, n)| (Sym::new(k), Value::Int(n)));
    Value::Map(fields.collect())
}

/// A reachable stop with what reaching it costs and causes, as the UI preview and the AI read it.
fn previewed(reach: &Reach) -> Value {
    let int = |n: i64| Value::Int(n);
    let s = reach.space;
    let path = reach.path.iter().map(|&at| space_value(at)).collect();
    let mut fields = vec![
        ("q", int(s.q.into())),
        ("r", int(s.r.into())),
        ("z", int(s.z.into())),
        ("cost", int(reach.cost)),
        ("path", Value::List(path)),
        ("stops_here", Value::Bool(reach.stops_here)),
        ("falls", Value::Bool(reach.falls)),
        ("provokes_reprisal", Value::Bool(reach.provokes_reprisal)),
    ];
    if reach.cells.len() > 1 {
        fields.push(("o", int(reach.rotation.steps().into())));
    }
    let fields = fields.into_iter().map(|(k, v)| (Sym::new(k), v));
    Value::Map(fields.collect())
}

/// The stop `move` asked for: the given rotation, or else the cheapest one.
fn chosen(reach: Vec<Reach>, to: Space, turn: Option<Rotation>) -> Option<Reach> {
    let mut hits = stops(reach).into_iter().filter(|r| r.space == to);
    match turn {
        Some(turn) => hits.find(|r| r.rotation == turn),
        None => hits.min_by_key(|r| (r.cost, r.rotation)),
    }
}

/// The spaces a figure may stop on: reachable and not occupied.
fn stops(reach: Vec<Reach>) -> Vec<Reach> {
    reach.into_iter().filter(|r| !r.occupied).collect()
}

impl ApiHandler for Api {
    fn call(
        &self,
        ctx: &mut dyn Ctx,
        env: &dyn NativeEnv,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Halt> {
        let who = figure(args)?;
        let reach = env
            .reachable(ctx.state(), who)
            .map_err(|e| Halt::reject(e.0))?;
        match name {
            "reachable" => Ok(Value::List(stops(reach).iter().map(listed).collect())),
            "preview" => Ok(Value::List(stops(reach).iter().map(previewed).collect())),
            "move" => {
                let to = space(args)?;
                let found = chosen(reach, to, rotation(args)?);
                let found =
                    found.ok_or_else(|| Halt::reject("that figure cannot reach that space"))?;
                let state = ctx.state();
                let from = state.figures[who.index()].location;
                let before = found.path.iter().rev().nth(1).copied().or(from);
                // Reprisals resolve first, and a figure they take out of play does not move.
                ctx.leave_engagement(who, &found.walk)?;
                if ctx.state().standing(who).is_none() {
                    return Ok(Value::Nil);
                }
                let moved = Event::FigureMoved {
                    figure: who,
                    to,
                    rotation: found.rotation,
                    path: found.path,
                };
                ctx.emit(moved)?;
                if !found.falls {
                    return Ok(Value::Nil);
                }
                let levels = before.map_or(0, |b| b.z.saturating_sub(to.z));
                let payload = [
                    (Sym::new("figure"), Value::Int(i64::from(who.0))),
                    (Sym::new("levels"), Value::Int(i64::from(levels))),
                ];
                let fell = Event::Custom {
                    id: Sym::new(FIGURE_FELL),
                    payload: Value::Map(payload.into()),
                };
                ctx.emit(fell).map(|()| Value::Nil)
            }
            other => Err(Halt::reject(format!("ctx.movement has no `{other}`"))),
        }
    }
}
