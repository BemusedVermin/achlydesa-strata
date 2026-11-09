//! `toy:driver`: turns the `attack` command into an `attack_declared` event.

use strata_kernel::{ModuleId, Rotation, SlotId, Space, Sym, Value};
use strata_model::{Command, Event, FigureId, PlayerId};
use strata_rules::{CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, Registrar, RuleModule};

use super::{ATTACK_DECLARED, no_config};

/// Handles `attack`: its payload becomes the payload of the declared attack.
struct Attack;

impl CommandHandler for Attack {
    fn kind(&self) -> &str {
        "attack"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("`attack` is a custom command"));
        };
        ctx.emit(Event::Custom {
            id: Sym::new(ATTACK_DECLARED),
            payload: payload.clone(),
        })
    }
}

/// Handles `move_to`, a stand-in for the movement action in tests: the payload `{ figure, q, r, z }`
/// puts the figure there, whatever the terrain, in one step or through the spaces of an optional
/// `via` list of `{ q, r, z }`. A figure covering more than its anchor lists every space it covers
/// there, anchor included and in offset order, as `cells`; without it the figure covers `to`
/// alone. Like a move, it first resolves the reprisals for leaving engagement along that path,
/// and stays put if they take the figure out of play.
struct MoveTo;

impl CommandHandler for MoveTo {
    fn kind(&self) -> &str {
        "move_to"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, command: &Command) -> Result<(), Halt> {
        let Command::Custom { payload, .. } = command else {
            return Err(Halt::reject("`move_to` is a custom command"));
        };
        let int = |at: &Value, name: &str| {
            let found = at.get(name).and_then(Value::as_int);
            found.map_err(|e| Halt::reject(format!("move_to {name}: {e}")))
        };
        let space = |at: &Value| -> Result<Space, Halt> {
            let (q, r, z) = (int(at, "q")?, int(at, "r")?, int(at, "z")?);
            Ok(Space::new(q as i32, r as i32, z as u32))
        };
        let figure = FigureId(int(payload, "figure")? as u32);
        let via = payload.as_map().ok().and_then(|m| m.get("via"));
        let via = via.map_or(Ok(&[][..]), Value::as_list);
        let via = via.map_err(|e| Halt::reject(format!("move_to via: {e}")))?;
        let mut path = via.iter().map(space).collect::<Result<Vec<_>, _>>()?;
        let to = space(payload)?;
        path.push(to);
        let start = ctx.state().standing(figure);
        let moving = start.is_some();
        // A one-space walk: where the figure stands, then each space of the path.
        let walk: Vec<Vec<Space>> = start.into_iter().chain(path).map(|s| vec![s]).collect();
        ctx.leave_engagement(figure, &walk)?;
        if moving && ctx.state().standing(figure).is_none() {
            return Ok(());
        }
        let cells = payload.as_map().ok().and_then(|m| m.get("cells"));
        let cells = match cells.map(Value::as_list).transpose() {
            Ok(Some(listed)) => listed.iter().map(space).collect::<Result<Vec<_>, _>>()?,
            Ok(None) => vec![to],
            Err(e) => return Err(Halt::reject(format!("move_to cells: {e}"))),
        };
        let rotation = Rotation::default();
        ctx.emit(Event::FigureMoved {
            figure,
            to,
            rotation,
            path: Vec::new(),
            cells,
        })
    }
}

/// The add-on every toy ruleset shares, whichever combat module it uses.
pub struct Driver;

impl RuleModule for Driver {
    fn id(&self) -> ModuleId {
        ModuleId::new("toy:driver")
    }

    fn slot(&self) -> Option<SlotId> {
        None
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        no_config()
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.command(Attack);
        r.command(MoveTo);
        Ok(())
    }
}
