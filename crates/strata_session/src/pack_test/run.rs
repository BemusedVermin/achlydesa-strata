//! Running one test: resolve the ruleset, place the figures, then play the steps.

use std::sync::Arc;

use strata_kernel::DefId;
use strata_lua::{LuaRules, ModuleBinding};
use strata_model::board::compile;
use strata_model::{
    Command, Event, FigureId, FigureStatus, MapDef, RuleModuleDef, RulesetDef, Step, TestDef,
};
use strata_rules::{
    Engine, LineOfSight, Outcome, RulesetSource, SourcedModifier, event_kind, resolve,
};

use super::host::{Host, RegistryDefs, find};
use super::report::Failure;
use super::{PackTests, state};
n/// Finds a test's rulesets by name, preferring its own pack.
pub(super) struct Rulesets<'a> {
    pub(super) tests: &'a PackTests,
    pub(super) pack: &'a str,
}

impl RulesetSource for Rulesets<'_> {
    fn ruleset(&self, name: &str) -> Option<&RulesetDef> {
        let found = find::<RulesetDef>(&self.tests.registry, self.pack, name);
        found.map(|(_, def)| def)
    }
}

/// The figure an event is about, if it is about one.
pub(super) fn event_figure(event: &Event) -> Option<FigureId> {
    match event {
        Event::FigureMoved { figure, .. }
        | Event::WoundsApplied { figure, .. }
        | Event::MarkerChanged { figure, .. }
        | Event::ControlChanged { figure, .. }
        | Event::FigureDestroyed(figure) => Some(*figure),
        _ => None,
    }
}

/// The events a command committed (none while it waits on a decision), or why it was refused.
fn settle(outcome: Outcome, what: &str) -> Result<Vec<Event>, String> {
    match outcome {
        Outcome::Committed(events) => Ok(events),
        Outcome::NeedsDecision { .. } => Ok(Vec::new()),
        Outcome::Rejected(violation) => Err(format!("`{what}` was rejected: {violation}")),
    }
}

pub(super) fn status_name(status: FigureStatus) -> &'static str {
    match status {
        FigureStatus::Reserve => "reserve",
        FigureStatus::OnBoard => "on_board",
        FigureStatus::Destroyed => "destroyed",
        FigureStatus::Removed => "removed",
    }
}

impl PackTests {
    /// Runs the test `id`, stopping at the first step that fails. Every committed command's
    /// events are appended to `log`. On success, returns the state
    /// hash after each committed command.
    pub(super) fn run_one(
        &self,
        id: &str,
        def: &TestDef,
        log: &mut Vec<Event>,
    ) -> Result<Vec<u64>, Failure> {
        let setup = |message: String| Failure {
            step: None,
            message,
        };
        let pack = id.split_once(':').map_or("", |(pack, _)| pack);
        let name = def
            .ruleset
            .as_ref()
            .ok_or_else(|| setup("the test names no ruleset".into()))?;
        let board = match &def.map {
            Some(map) => {
                let (_, found) = find::<MapDef>(&self.registry, pack, map.as_str())
                    .ok_or_else(|| setup(format!("unknown map `{map}`")))?;
                let defs = RegistryDefs {
                    registry: &self.registry,
                    pack,
                };
                Some(compile(found, &defs).map_err(|e| setup(e.to_string()))?)
            }
            None => None,
        };
        let rulesets = Rulesets { tests: self, pack };
        let rt =
            resolve(name.as_str(), &rulesets, &self.catalog).map_err(|e| setup(e.to_string()))?;
        let (ruleset_id, _) = find::<RulesetDef>(&self.registry, pack, name.as_str())
            .ok_or_else(|| setup(format!("unknown ruleset `{name}`")))?;
        let start = state::build(&self.registry, pack, ruleset_id, def).map_err(setup)?;

        let modifiers = rt.modifiers().iter().map(|m| SourcedModifier {
            source: name.clone(),
            priority: 0,
            def: m.clone(),
        });
        let los = rt.service::<Arc<dyn LineOfSight>>().cloned();
        let host = Host::new(&self.registry, pack, modifiers.collect(), board).with_los(los);
        let host = Arc::new(host);
        let lua_modules = rt
            .configs()
            .iter()
            .filter(|(id, _)| self.registry.find::<RuleModuleDef>(id.as_str()).is_some())
            .map(|(id, config)| ModuleBinding {
                id: id.as_str().to_owned(),
                config: config.clone(),
            });
        let scripts = self.registry.scripts();
        let packs = self.packs.clone();
        let rules = LuaRules::new(&rt, scripts, packs, lua_modules.collect(), host.clone());
        let rules = Arc::new(rules);
        let _ = host.rules.set(Arc::downgrade(&rules));
        let mut engine = Engine::new(&rt, &*rules, start);

        let mut events: Vec<Event> = Vec::new();
        for (index, step) in def.steps.iter().enumerate() {
            let at = |message: String| Failure {
                step: Some(index + 1),
                message,
            };
            let figure = |handle: &str| state::figure(engine.state(), &self.registry, handle);
            match step {
                Step::Custom {
                    player,
                    id,
                    payload,
                    rig,
                } => {
                    host.set_rig(rig.clone());
                    let command = Command::Custom {
                        id: id.clone(),
                        payload: payload.clone(),
                    };
                    events = settle(engine.submit(*player, command), id.as_str()).map_err(at)?;
                    log.extend(events.iter().cloned());
                }
                Step::Action {
                    figure: handle,
                    action,
                    args,
                    rig,
                } => {
                    let found = figure(handle).map_err(at)?;
                    let position = rt.actions().iter().position(|a| a.id == *action);
                    let position =
                        position.ok_or_else(|| at(format!("unknown action `{action}`")))?;
                    host.set_rig(rig.clone());
                    let command = Command::Action {
                        figure: found,
                        action: DefId(position as u32),
                        args: args.clone(),
                    };
                    let player = engine.state().figures[found.index()].controller;
                    events = settle(engine.submit(player, command), action.as_str()).map_err(at)?;
                    log.extend(events.iter().cloned());
                }
                Step::Answer(answer) => {
                    host.rewind();
                    events = settle(engine.answer(answer.clone()), "answer").map_err(at)?;
                    log.extend(events.iter().cloned());
                }
                Step::ExpectStat {
                    figure: handle,
                    stat,
                    value,
                } => {
                    let found = figure(handle).map_err(at)?;
                    let got = host.stat(engine.state(), found, stat.as_str());
                    let got = got.map_err(|e| at(e.to_string()))?;
                    if got != *value {
                        return Err(at(format!("{handle} {stat}: expected {value}, got {got}")));
                    }
                }
                Step::ExpectEvent {
                    kind,
                    figure: handle,
                } => {
                    let about = handle.as_deref().map(figure).transpose().map_err(at)?;
                    let hit = events.iter().any(|e| {
                        event_kind(e) == kind.as_str()
                            && about.is_none_or(|a| event_figure(e) == Some(a))
                    });
                    if !hit {
                        let got: Vec<&str> = events.iter().map(event_kind).collect();
                        let of = handle
                            .as_deref()
                            .map_or(String::new(), |h| format!(" for {h}"));
                        return Err(at(format!("event `{kind}`{of}: expected it, got {got:?}")));
                    }
                }
                Step::ExpectFigure {
                    figure: handle,
                    status,
                } => {
                    let found = figure(handle).map_err(at)?;
                    let got = status_name(engine.state().figures[found.index()].status);
                    if got != status.as_str() {
                        return Err(at(format!("{handle}: expected {status}, got {got}")));
                    }
                }
                Step::ExpectDecision { prompt } => {
                    let pending = engine.state().pending.as_ref();
                    let got = pending.map(|p| p.decision.prompt.as_str());
                    if got != Some(prompt.as_str()) {
                        return Err(at(format!("decision: expected {prompt}, got {got:?}")));
                    }
                }
            }
        }
        Ok(engine.log().iter().map(|r| r.state_hash).collect())
    }
}
