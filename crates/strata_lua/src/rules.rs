//! Runs the Lua actions, triggers, replacements and reactions a ruleset registered (design §6.2).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use mlua::{Lua, LuaSerdeExt, Value as LuaValue};
use strata_kernel::{CommandRng, DefId, Sym, Value, WindowId};
use strata_model::{
    ActionDef, Answer, Decision, DieKind, Event, FigureId, GameState, Logic, PlayerId, ReactionDef,
    ReplacementDef, ScriptRef, TriggerDef,
};
use strata_rules::{
    ActionRunner, ApiHandler, Ctx, Halt, HookRunner, LogicError, NativeEnv, PhaseTree, Reach,
    RulesRuntime,
};

use crate::convert::{lua_to_value, value_to_lua};
use crate::ctx::{Args, Frame, Namespaces, RuleHost};
use crate::data::PackSource;
use crate::runtime::{ModuleBinding, RuntimeLua};

static NEXT: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// The runtime state of each `LuaRules`, built the first time the thread uses it.
    static STATES: RefCell<BTreeMap<u64, RuntimeLua>> = const { RefCell::new(BTreeMap::new()) };
}

/// Runs registered Lua logic for the engine.
///
/// A Lua state cannot cross threads, so this holds only what builds one. Each thread that calls
/// it builds its own state from the packs the first time, and keeps it for the thread's life.
pub struct LuaRules {
    token: u64,
    scripts: Vec<ScriptRef>,
    packs: Vec<PackSource>,
    modules: Vec<ModuleBinding>,
    pub(crate) host: Arc<dyn RuleHost>,
    pub(crate) namespaces: Namespaces,
    /// The Rust behind the namespaces that modules exported with `lua_api_native`.
    pub(crate) handlers: BTreeMap<String, Arc<dyn ApiHandler>>,
    /// The phases of the ruleset, for native logic that moves between them.
    phases: PhaseTree,
}

impl LuaRules {
    /// Prepares to run the logic in `rt`. `scripts` are the registry's `ScriptRef`s, `modules`
    /// the Lua modules of the ruleset with their configs, and `rt` supplies the `ctx`
    /// namespaces its modules exported.
    pub fn new(
        rt: &RulesRuntime,
        scripts: Vec<ScriptRef>,
        packs: Vec<PackSource>,
        modules: Vec<ModuleBinding>,
        host: Arc<dyn RuleHost>,
    ) -> Self {
        let names = |api: &strata_rules::LuaApi| {
            api.functions
                .iter()
                .map(|f| f.as_str().to_owned())
                .collect()
        };
        let namespaces = rt
            .lua_apis()
            .map(|(ns, api)| (ns.as_str().to_owned(), names(api)));
        let handlers = rt.lua_apis().filter_map(|(ns, _)| {
            let handler = rt.api_handler(ns.as_str())?;
            Some((ns.as_str().to_owned(), handler))
        });
        Self {
            handlers: handlers.collect(),
            phases: PhaseTree::build(rt.phases()).unwrap_or_default(),
            token: NEXT.fetch_add(1, Ordering::Relaxed),
            scripts,
            packs,
            modules,
            host,
            namespaces: namespaces.collect(),
        }
    }

    /// The runtime state for this thread, built on first use.
    pub fn with_state<R>(&self, f: impl FnOnce(&RuntimeLua) -> R) -> Result<R, Halt> {
        STATES.with(|states| {
            if !states.borrow().contains_key(&self.token) {
                let built = RuntimeLua::new(&self.scripts, &self.packs).and_then(|mut state| {
                    state.bind_modules(&self.modules)?;
                    Ok(state)
                });
                let state = built.map_err(|e| Halt::reject(e.to_string()))?;
                states.borrow_mut().insert(self.token, state);
            }
            Ok(f(&states.borrow()[&self.token]))
        })
    }

    /// Runs `logic` as a script with `args`, or as a native descriptor against no subject.
    fn value(&self, logic: &Logic, ctx: &mut dyn Ctx, args: Args<'_>) -> Result<Value, Halt> {
        match logic {
            Logic::Native(descriptor) => {
                let found = self.host.eval(ctx.state(), descriptor, &Value::Nil);
                found.map_err(|e| Halt::reject(e.0))
            }
            Logic::Script(script) => {
                let finish = |lua: &Lua, v: LuaValue| Ok(lua_to_value(lua, &v));
                let frame = Frame::new(ctx, self);
                let run = self.with_state(|rt| frame.invoke(rt, script, args, finish))?;
                run?.map_err(|e| Halt::reject(e.to_string()))
            }
        }
    }

    /// Whether the optional filter holds; no filter always holds.
    fn holds(
        &self,
        filter: &Option<Logic>,
        state: &GameState,
        event: &Event,
    ) -> Result<bool, Halt> {
        let Some(logic) = filter else { return Ok(true) };
        let mut view = View::new(state);
        let verdict = self.value(logic, &mut view, &with_event(event))?;
        verdict
            .as_bool()
            .map_err(|e| Halt::reject(format!("a filter must return a bool: {e}")))
    }
}

/// Hands scripts the event as `{ Variant = { ... } }`, except that the free-form `Value` an event
/// carries (a custom payload, a variable's value) is a plain Lua value.
fn event_to_lua(lua: &Lua, event: &Event) -> mlua::Result<LuaValue> {
    let tagged = lua.to_value(event)?;
    let plain = match event {
        Event::Custom { payload, .. } => Some(("Custom", "payload", payload)),
        Event::VarSet { value, .. } => Some(("VarSet", "value", value)),
        _ => None,
    };
    if let (Some((variant, field, value)), LuaValue::Table(outer)) = (plain, &tagged) {
        let inner: mlua::Table = outer.get(variant)?;
        inner.set(
            field,
            value_to_lua(lua, value).map_err(mlua::Error::external)?,
        )?;
    }
    Ok(tagged)
}

fn with_event(event: &Event) -> impl Fn(&Lua) -> mlua::Result<Vec<LuaValue>> + '_ {
    move |lua| Ok(vec![event_to_lua(lua, event)?])
}

impl ActionRunner for LuaRules {
    fn targets(
        &self,
        def: &ActionDef,
        figure: FigureId,
        state: &GameState,
    ) -> Result<Vec<Value>, Halt> {
        let Some(logic) = &def.targets else {
            return Ok(Vec::new());
        };
        let args = |_: &Lua| Ok(vec![LuaValue::Integer(i64::from(figure.0))]);
        match self.value(logic, &mut View::new(state), &args)? {
            Value::List(targets) => Ok(targets),
            other => Err(Halt::reject(format!(
                "targets must be a list, found {}",
                other.type_name()
            ))),
        }
    }

    fn resolve(
        &self,
        def: &ActionDef,
        figure: FigureId,
        args: &Value,
        ctx: &mut dyn Ctx,
    ) -> Result<(), Halt> {
        let spread = match args {
            Value::List(items) => items.clone(),
            other => vec![other.clone()],
        };
        let args = |lua: &Lua| {
            let mut all = vec![LuaValue::Integer(i64::from(figure.0))];
            for item in &spread {
                all.push(value_to_lua(lua, item).map_err(mlua::Error::external)?);
            }
            Ok(all)
        };
        self.value(&def.resolve, ctx, &args).map(drop)
    }
}

impl NativeEnv for LuaRules {
    fn stat(&self, state: &GameState, figure: FigureId, stat: &str) -> Result<i64, LogicError> {
        let args = [Value::Int(i64::from(figure.0)), Value::Str(Sym::new(stat))];
        let found = self.host.query(state, "stat", &args)?;
        found.as_int().map_err(|e| LogicError(e.to_string()))
    }

    fn die(&self, name: &str) -> Option<(DefId, DieKind)> {
        self.host.die(name)
    }

    fn phases(&self) -> Option<&PhaseTree> {
        Some(&self.phases)
    }

    fn marker(&self, name: &str) -> Option<DefId> {
        self.host.marker(name)
    }

    fn deployment(&self, state: &GameState, player: PlayerId) -> Option<strata_rules::DeploySlot> {
        self.host.deployment(state, player)
    }

    fn standable(&self, space: strata_kernel::Space) -> bool {
        self.host.standable(space)
    }

    fn relic(&self, id: DefId) -> Option<strata_model::RelicDef> {
        self.host.relic(id)
    }

    fn run_effect(&self, ctx: &mut dyn Ctx, effect: &Logic, event: &Event) -> Result<(), Halt> {
        self.value(effect, ctx, &with_event(event)).map(drop)
    }
    fn reachable(&self, state: &GameState, figure: FigureId) -> Result<Vec<Reach>, LogicError> {
        self.host.reachable(state, figure)
    }

    fn forced_roll(&self, die: &str, count: usize) -> Option<Result<Vec<u32>, String>> {
        self.host.forced_roll(die, count)
    }
}

impl HookRunner for LuaRules {
    fn action_runner(&self) -> Option<&dyn ActionRunner> {
        Some(self)
    }

    fn env(&self) -> &dyn NativeEnv {
        self
    }

    fn replaces(
        &self,
        def: &ReplacementDef,
        event: &Event,
        state: &GameState,
    ) -> Result<bool, Halt> {
        self.holds(&def.filter, state, event)
    }

    /// The script returns the events that take the event's place, in the event's own shape.
    fn replace(
        &self,
        def: &ReplacementDef,
        event: &Event,
        ctx: &mut dyn Ctx,
    ) -> Result<Vec<Event>, Halt> {
        let Logic::Script(script) = &def.replace else {
            return Err(Halt::reject("native replacements need the native runner"));
        };
        let frame = Frame::new(ctx, self);
        let finish = |lua: &Lua, v: LuaValue| lua.from_value::<Vec<Event>>(v);
        self.with_state(|rt| frame.invoke(rt, script, &with_event(event), finish))?
    }

    fn fires(&self, def: &TriggerDef, event: &Event, state: &GameState) -> Result<bool, Halt> {
        self.holds(&def.filter, state, event)
    }

    fn fire(&self, def: &TriggerDef, event: &Event, ctx: &mut dyn Ctx) -> Result<(), Halt> {
        self.value(&def.effect, ctx, &with_event(event)).map(drop)
    }

    /// The `when` script returns the player who may react, or nil for nobody.
    fn reactor(&self, def: &ReactionDef, state: &GameState) -> Result<Option<PlayerId>, Halt> {
        let Some(when) = &def.when else {
            return Ok(Some(state.flow.active));
        };
        match self.value(when, &mut View::new(state), &|_: &Lua| Ok(Vec::new()))? {
            Value::Int(player) => Ok(Some(PlayerId(player as u8))),
            Value::Bool(true) => Ok(Some(state.flow.active)),
            _ => Ok(None),
        }
    }

    fn react(&self, def: &ReactionDef, ctx: &mut dyn Ctx) -> Result<(), Halt> {
        self.value(&def.effect, ctx, &|_: &Lua| Ok(Vec::new()))
            .map(drop)
    }
}

/// A read-only `Ctx` for filters and targeting, which see the state but change nothing.
struct View<'a> {
    state: &'a GameState,
    rng: CommandRng,
}

impl<'a> View<'a> {
    fn new(state: &'a GameState) -> Self {
        Self {
            state,
            rng: CommandRng::for_command(0, 0),
        }
    }
}

impl Ctx for View<'_> {
    fn state(&self) -> &GameState {
        self.state
    }

    fn rng(&mut self) -> &mut CommandRng {
        &mut self.rng
    }

    fn emit(&mut self, _: Event) -> Result<(), Halt> {
        Err(Halt::reject("a filter cannot emit events"))
    }

    fn window(&mut self, _: &WindowId) -> Result<(), Halt> {
        Err(Halt::reject("a filter cannot open a window"))
    }

    fn choose(&mut self, _: PlayerId, _: Decision) -> Result<Answer, Halt> {
        Err(Halt::reject("a filter cannot ask a decision"))
    }
}
