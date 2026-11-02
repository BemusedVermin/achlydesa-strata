//! The host side of `ctx`: what a script may ask of the running command (design §9.2).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use mlua::{Lua, MultiValue, Value as LuaValue};
use strata_kernel::{ModuleId, Space, Sym, Value};
use strata_model::{Descriptor, Event, FigureStatus, GameState, ScriptRef, VarKey};
use strata_rules::{Ctx, Halt, LogicError};

use crate::convert::{lua_to_value, value_to_lua};
use crate::error::HostError;
use crate::runtime::RuntimeLua;

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
    /// Calls `name` of the namespace a module exported with `Registrar::lua_api`.
    fn call(
        &self,
        namespace: &str,
        name: &str,
        args: &[Value],
        ctx: &mut dyn Ctx,
    ) -> Result<Value, Halt>;
    /// Receives `ctx.log` output. It must not affect the game.
    fn log(&self, _text: &str) {}
}

/// The namespaces modules exported, with the functions each one offers.
pub(crate) type Namespaces = BTreeMap<String, Vec<String>>;

/// One script call: the command's `Ctx`, the host and whatever stopped the call.
pub(crate) struct Frame<'a> {
    ctx: RefCell<&'a mut dyn Ctx>,
    host: &'a Arc<dyn RuleHost>,
    namespaces: &'a Namespaces,
    halt: RefCell<Option<Halt>>,
}

/// Arguments for a script, built once the call's Lua state is known.
pub(crate) type Args<'a> = &'a dyn Fn(&Lua) -> mlua::Result<Vec<LuaValue>>;

impl<'a> Frame<'a> {
    pub(crate) fn new(
        ctx: &'a mut dyn Ctx,
        host: &'a Arc<dyn RuleHost>,
        namespaces: &'a Namespaces,
    ) -> Self {
        Self {
            ctx: RefCell::new(ctx),
            host,
            namespaces,
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
            for (name, functions) in self.namespaces {
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
                self.host.call(namespace, name, list, ctx)
            }
            "log" => {
                self.host.log(args.as_str().map_err(bad)?);
                Ok(Value::Nil)
            }
            other => Err(Halt::reject(format!("unknown ctx operation `{other}`"))),
        }
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
