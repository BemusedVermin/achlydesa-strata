//! `ctx.movement`: `reachable(figure)` lists where a figure may stop, `move(figure, q, r, z)`
//! walks it there.

use strata_kernel::{Space, Value, ValueError};
use strata_model::{Event, FigureId};
use strata_rules::{ApiHandler, Ctx, Halt, NativeEnv, Reach};

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
            "reachable" => {
                let at = |s: Space| {
                    Value::List(
                        vec![s.q, s.r, s.z as i32]
                            .into_iter()
                            .map(|n| Value::Int(i64::from(n)))
                            .collect(),
                    )
                };
                Ok(Value::List(
                    stops(reach).iter().map(|r| at(r.space)).collect(),
                ))
            }
            "move" => {
                let to = space(args)?;
                let found = stops(reach).into_iter().find(|r| r.space == to);
                let found =
                    found.ok_or_else(|| Halt::reject("that figure cannot reach that space"))?;
                let rotation = ctx.state().figures[who.index()].rotation;
                let moved = Event::FigureMoved {
                    figure: who,
                    to,
                    rotation,
                    path: found.path,
                };
                ctx.emit(moved).map(|()| Value::Nil)
            }
            other => Err(Halt::reject(format!("ctx.movement has no `{other}`"))),
        }
    }
}
