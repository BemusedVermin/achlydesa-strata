//! The host side of `ctx`: what a script may ask of the running command (design §9.2).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use mlua::{Lua, MultiValue, Value as LuaValue};
use strata_kernel::{ModuleId, Space, Sym, Value};
use strata_model::{
    Answer, Decision, DecisionKind, Descriptor, DieKind, Event, FigureId, FigureStatus, GameState,
    PlayerId, ScriptRef, VarKey,
};
use strata_rules::{Ctx, Halt, LogicError, Reach};

use crate::convert::{lua_to_value, value_to_lua};
use crate::error::HostError;
use crate::rules::LuaRules;
use crate::runtime::RuntimeLua;

/// The most dice one `ctx.dice.roll` may throw.
const MAX_DICE: i64 = 1024;

/// What `ctx` needs from the application that owns the content registry.
pub trait RuleHost: Send + Sync {
    /// Evaluates a native descriptor, such as a selector, for `subject`.
    fn eval(
        &self,
        state: &GameState,
        descriptor: &Descriptor,
        subject: &Value,
    ) -> Result<Value, LogicError>;
    /// Runs the query `name` with every modifier applied, such as `stat` with `[figure, "defense"]`.
    fn query(&self, state: &GameState, name: &str, args: &[Value]) -> Result<Value, LogicError>;
    /// The definition and faces of the die with this name.
    fn die(&self, name: &str) -> Option<(strata_kernel::DefId, DieKind)>;
    /// The definition of the marker with this name, such as `wound`.
    fn marker(&self, _name: &str) -> Option<strata_kernel::DefId> {
        None
    }
    /// The relic with this definition ID.
    fn relic(&self, _id: strata_kernel::DefId) -> Option<strata_model::RelicDef> {
        None
    }
    /// Every space `figure` can reach under the ruleset's `movement` module.
    fn reachable(&self, _state: &GameState, _figure: FigureId) -> Result<Vec<Reach>, LogicError> {
        Err(LogicError("the ruleset has no `movement` module".into()))
    }
    /// Calls `name` of the namespace a module exported with `Registrar::lua_api`.
    fn call(
        &self,
        namespace: &str,
        name: &str,
        args: &[Value],
        ctx: &mut dyn Ctx,
    ) -> Result<Value, Halt>;
    /// Results that replace the next roll of `count` dice named `die`, for tests that rig the
    /// dice: one-based face numbers for a die with faces, the values themselves for a range.
    /// `None` leaves the roll to the command's random stream.
    fn forced_roll(&self, _die: &str, _count: usize) -> Option<Result<Vec<u32>, String>> {
        None
    }
    /// Receives `ctx.log` output. It must not affect the game.
    fn log(&self, _text: &str) {}
}

/// The namespaces modules exported, with the functions each one offers.
pub(crate) type Namespaces = BTreeMap<String, Vec<String>>;

/// One script call: the command's `Ctx`, the host and whatever stopped the call.
pub(crate) struct Frame<'a> {
    ctx: RefCell<&'a mut dyn Ctx>,
    host: &'a Arc<dyn RuleHost>,
    rules: &'a LuaRules,
    halt: RefCell<Option<Halt>>,
}

/// Arguments for a script, built once the call's Lua state is known.
pub(crate) type Args<'a> = &'a dyn Fn(&Lua) -> mlua::Result<Vec<LuaValue>>;

impl<'a> Frame<'a> {
    pub(crate) fn new(ctx: &'a mut dyn Ctx, rules: &'a LuaRules) -> Self {
        Self {
            ctx: RefCell::new(ctx),
            host: &rules.host,
            rules,
            halt: RefCell::new(None),
        }
    }

    /// Calls `script` with a fresh `ctx` followed by `args`, and maps its result with `finish`.
    ///
    /// A halt raised through `ctx` comes back as itself; any other failure rejects the command.
    pub(crate) fn invoke<R>(
        &self,
        rt: &RuntimeLua,
        script: &ScriptRef,
        args: Args<'_>,
        finish: impl FnOnce(&Lua, LuaValue) -> mlua::Result<R>,
    ) -> Result<R, Halt> {
        let lua = rt.lua();
        let run = lua.scope(|scope| {
            let op = scope
                .create_function(|lua, (name, a): (String, LuaValue)| self.op(lua, &name, &a))?;
            let namespaces = lua.create_table()?;
            for (name, functions) in &self.rules.namespaces {
                namespaces.raw_set(name.as_str(), functions.clone())?;
            }
            let ctx: LuaValue = rt.ctx_builder().call((op, namespaces))?;
            let mut all = vec![ctx];
            all.extend(args(lua)?);
            match rt.call_raw(script, MultiValue::from_iter(all)) {
                Ok(mut out) => finish(lua, out.pop_front().unwrap_or(LuaValue::Nil)).map(Ok),
                Err(error) => Ok(Err(error)),
            }
        });
        if let Some(halt) = self.halt.take() {
            return Err(halt);
        }
        match run {
            Ok(Ok(done)) => Ok(done),
            Ok(Err(HostError::Halt)) => Err(Halt::reject("script halted")),
            Ok(Err(error)) => Err(Halt::reject(error.to_string())),
            Err(error) => Err(Halt::reject(error.to_string())),
        }
    }

    /// The one function behind every `ctx` entry point. A halt is parked and reported to Lua as
    /// `false`, so the script is abandoned without being able to catch it.
    fn op(&self, lua: &Lua, name: &str, args: &LuaValue) -> mlua::Result<(bool, LuaValue)> {
        let external = mlua::Error::external;
        let args = lua_to_value(lua, args).map_err(external)?;
        match self.dispatch(name, &args) {
            Ok(value) => Ok((true, value_to_lua(lua, &value).map_err(external)?)),
            Err(halt) => {
                *self.halt.borrow_mut() = Some(halt);
                Ok((false, LuaValue::Nil))
            }
        }
    }

    fn dispatch(&self, op: &str, args: &Value) -> Result<Value, Halt> {
        let mut guard = self.ctx.borrow_mut();
        let ctx: &mut dyn Ctx = &mut **guard;
        let bad = |e: strata_kernel::ValueError| Halt::reject(format!("ctx.{op}: {e}"));
        let field = |key: &str| args.get(key).map_err(bad);
        match op {
            "state" => state_view(ctx.state(), args.as_str().map_err(bad)?),
            "select" => {
                let selector = Descriptor(field("selector")?.clone());
                let subject = args.get("subject").cloned().unwrap_or_default();
                let found = self.host.eval(ctx.state(), &selector, &subject);
                match found.map_err(|e| Halt::reject(e.0))? {
                    Value::List(mut ids) => {
                        ids.sort();
                        Ok(Value::List(ids))
                    }
                    other => Ok(other),
                }
            }
            "query" => {
                let name = field("name")?.as_str().map_err(bad)?;
                let list = field("args")?.as_list().map_err(bad)?;
                let asked = self.host.query(ctx.state(), name, list);
                asked.map_err(|e| Halt::reject(e.0))
            }
            "choose" => choose(ctx, args),
            "roll" => self.roll(ctx, field("die")?.as_str().map_err(bad)?, field("n")?),
            "var_get" => Ok(ctx
                .state()
                .vars
                .get(&var_key(args).map_err(bad)?)
                .cloned()
                .unwrap_or_default()),
            "var_set" => {
                let value = args.get("value").cloned().unwrap_or_default();
                let key = var_key(args).map_err(bad)?;
                ctx.emit(Event::VarSet { key, value }).map(|()| Value::Nil)
            }
            "emit" => {
                let id = Sym::new(field("kind")?.as_str().map_err(bad)?);
                let payload = args.get("data").cloned().unwrap_or_default();
                ctx.emit(Event::Custom { id, payload }).map(|()| Value::Nil)
            }
            "api" => {
                let namespace = field("namespace")?.as_str().map_err(bad)?;
                let name = field("name")?.as_str().map_err(bad)?;
                let list = field("args")?.as_list().map_err(bad)?;
                match self.rules.handlers.get(namespace) {
                    Some(handler) => handler.call(ctx, self.rules, name, list),
                    None => self.host.call(namespace, name, list, ctx),
                }
            }
            "log" => {
                self.host.log(args.as_str().map_err(bad)?);
                Ok(Value::Nil)
            }
            other => Err(Halt::reject(format!("unknown ctx operation `{other}`"))),
        }
    }

    /// Rolls `n` dice from the command's stream and emits `DiceRolled`.
    fn roll(&self, ctx: &mut dyn Ctx, name: &str, n: &Value) -> Result<Value, Halt> {
        let (die, kind) = self
            .host
            .die(name)
            .ok_or_else(|| Halt::reject(format!("unknown die `{name}`")))?;
        let count = n.as_int().ok().filter(|n| (0..=MAX_DICE).contains(n));
        let count = count.ok_or_else(|| Halt::reject("dice count must be 0..=1024"))?;
        let drawn: Vec<u32> = (0..count)
            .map(|_| match &kind {
                DieKind::Faces(faces) => ctx.rng().roll(faces.len().max(1) as u32),
                DieKind::Range(low, high) => {
                    let span = (i64::from(*high) - i64::from(*low) + 1).max(1) as u32;
                    (i64::from(*low) + i64::from(ctx.rng().bounded(span))).max(0) as u32
                }
            })
            .collect();
        let results = match self.host.forced_roll(name, drawn.len()) {
            Some(forced) => forced.map_err(Halt::reject)?,
            None => drawn,
        };
        let faces = match &kind {
            DieKind::Faces(faces) => results
                .iter()
                .map(|r| Value::Str(faces[*r as usize - 1].clone()))
                .collect(),
            DieKind::Range(..) => results.iter().map(|r| Value::Int(i64::from(*r))).collect(),
        };
        ctx.emit(Event::DiceRolled {
            die,
            results: results.clone(),
        })?;
        let ints = results.iter().map(|r| Value::Int(i64::from(*r))).collect();
        let pair = [
            ("results", Value::List(ints)),
            ("faces", Value::List(faces)),
        ];
        Ok(Value::Map(pair.map(|(k, v)| (Sym::new(k), v)).into()))
    }
}

fn var_key(args: &Value) -> Result<VarKey, strata_kernel::ValueError> {
    Ok(VarKey {
        module: ModuleId::new(args.get("owner")?.as_str()?),
        name: Sym::new(args.get("name")?.as_str()?),
    })
}

fn map<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Map(entries.map(|(k, v)| (Sym::new(k), v)).into())
}

fn int(n: impl Into<i64>) -> Value {
    Value::Int(n.into())
}

/// A read-only copy of one part of the state.
fn state_view(state: &GameState, name: &str) -> Result<Value, Halt> {
    let flow = &state.flow;
    Ok(match name {
        "round" => int(flow.round),
        "turn" => int(flow.turn),
        "active" => int(flow.active.0),
        "phases" => Value::List(flow.phases.iter().map(|p| int(p.phase.0)).collect()),
        "units" => Value::List(
            (0u32..)
                .zip(&state.units)
                .map(|(id, unit)| {
                    let figures = unit.figures.iter().map(|f| int(f.0)).collect();
                    let owner = int(unit.owner.0);
                    map([
                        ("id", int(id)),
                        ("owner", owner),
                        ("figures", Value::List(figures)),
                    ])
                })
                .collect(),
        ),
        "figures" => Value::List(
            (0u32..)
                .zip(&state.figures)
                .map(|(id, f)| {
                    let status = match f.status {
                        FigureStatus::Reserve => "reserve",
                        FigureStatus::OnBoard => "on_board",
                        FigureStatus::Destroyed => "destroyed",
                        FigureStatus::Removed => "removed",
                    };
                    let at = |s: Space| map([("q", int(s.q)), ("r", int(s.r)), ("z", int(s.z))]);
                    let location = f.location.map_or(Value::Nil, at);
                    map([
                        ("id", int(id)),
                        ("unit", int(f.unit.0)),
                        ("controller", int(f.controller.0)),
                        ("status", Value::Str(Sym::new(status))),
                        ("location", location),
                    ])
                })
                .collect(),
        ),
        other => return Err(Halt::reject(format!("ctx.state has no `{other}`"))),
    })
}

/// Asks a decision described by `spec` and returns the answer.
///
/// `spec` has `chooser` and `prompt`, plus one of `figures`, `spaces`, `options`, `yes_no` or
/// `number = { min, max }`. An option answer is the option's label.
fn choose(ctx: &mut dyn Ctx, spec: &Value) -> Result<Value, Halt> {
    let bad = |e: strata_kernel::ValueError| Halt::reject(format!("ctx.choose: {e}"));
    let chooser = PlayerId(spec.get("chooser").and_then(Value::as_int).map_err(bad)? as u8);
    let prompt = Sym::new(spec.get("prompt").and_then(Value::as_str).map_err(bad)?);
    let list = |key: &str| spec.get(key).and_then(Value::as_list);
    let numbers =
        |items: &[Value]| -> Result<Vec<i64>, _> { items.iter().map(Value::as_int).collect() };
    let mut labels = Vec::new();
    let kind = if let Ok(items) = list("figures") {
        let ids = numbers(items).map_err(bad)?;
        DecisionKind::ChooseFigure(ids.into_iter().map(|i| FigureId(i as u32)).collect())
    } else if let Ok(items) = list("spaces") {
        let at = |s: &Value| -> Result<Space, strata_kernel::ValueError> {
            let get = |k: &str| s.get(k).and_then(Value::as_int);
            Ok(Space::new(
                get("q")? as i32,
                get("r")? as i32,
                get("z")? as u32,
            ))
        };
        DecisionKind::ChooseSpace(
            items
                .iter()
                .map(at)
                .collect::<Result<_, _>>()
                .map_err(bad)?,
        )
    } else if let Ok(items) = list("options") {
        labels = items
            .iter()
            .map(|i| i.as_str().map(Sym::new))
            .collect::<Result<_, _>>()
            .map_err(bad)?;
        DecisionKind::ChooseOption(labels.clone())
    } else if spec.get("yes_no").is_ok() {
        DecisionKind::YesNo
    } else if let Ok(range) = spec.get("number") {
        let get = |k: &str| range.get(k).and_then(Value::as_int);
        DecisionKind::Number {
            min: get("min").map_err(bad)? as i32,
            max: get("max").map_err(bad)? as i32,
        }
    } else {
        return Err(Halt::reject("ctx.choose: the decision names no kind"));
    };
    Ok(match ctx.choose(chooser, Decision { prompt, kind })? {
        Answer::Figure(f) => int(f.0),
        Answer::Space(s) => map([("q", int(s.q)), ("r", int(s.r)), ("z", int(s.z))]),
        Answer::Option(i) => labels
            .get(i as usize)
            .cloned()
            .map_or(Value::Nil, Value::Str),
        Answer::YesNo(b) => Value::Bool(b),
        Answer::Number(n) => int(n),
        Answer::Order(_) => return Err(Halt::reject("ctx.choose cannot order items")),
    })
}
