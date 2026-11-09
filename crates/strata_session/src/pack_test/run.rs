//! Running one test: resolve the ruleset, place the figures, then play the steps.

use std::sync::Arc;

use strata_content::RegistryDefs;
use strata_kernel::DefId;
use strata_lua::{LuaRules, ModuleBinding};
use strata_model::board::compile;
use strata_model::{
    Command, Event, FigureId, FigureStatus, MapDef, PlayerId, PlayerSetup, RuleModuleDef,
    RulesetDef, ScenarioDef, Step, TestDef, TokenAt, TokenPlacement,
};
use strata_rules::{
    Engagement, Engine, LineOfSight, Movement, Outcome, RulesetSource, SourcedModifier, event_kind,
    resolve,
};

use super::host::{Host, find};
use super::report::Failure;
use super::{PackTests, state};
use crate::setup::place_relics;

/// Finds a test's rulesets by name, preferring its own pack.
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
        let scenario = match &def.scenario {
            Some(s) => Some(
                find::<ScenarioDef>(&self.registry, pack, s.as_str())
                    .ok_or_else(|| setup(format!("unknown scenario `{s}`")))?,
            ),
            None => None,
        };
        let map = def.map.as_ref().or(scenario.map(|(_, s)| &s.map));
        let map = match map {
            Some(map) => Some(
                find::<MapDef>(&self.registry, pack, map.as_str())
                    .ok_or_else(|| setup(format!("unknown map `{map}`")))?
                    .1,
            ),
            None => None,
        };
        let defs = RegistryDefs::new(&self.registry, pack);
        let board = map.map(|m| compile(m, &defs).map_err(|e| setup(e.to_string())));
        let board = board.transpose()?;
        let rulesets = Rulesets { tests: self, pack };
        let rt =
            resolve(name.as_str(), &rulesets, &self.catalog).map_err(|e| setup(e.to_string()))?;
        let (ruleset_id, _) = find::<RulesetDef>(&self.registry, pack, name.as_str())
            .ok_or_else(|| setup(format!("unknown ruleset `{name}`")))?;
        let mut start =
            state::build(&self.registry, pack, ruleset_id, def, board.as_ref()).map_err(setup)?;
        if let Some((scenario_id, scenario)) = scenario {
            // Seat `i` fills the scenario's `i`-th slot, so deployment finds its start zone.
            let mut made = (*start.setup).clone();
            made.scenario = scenario_id;
            let slots = scenario.slots.iter().take(start.players.len());
            let seat = |(seat, slot): (u8, &strata_model::ScenarioSlot)| PlayerSetup {
                id: PlayerId(seat),
                slot: slot.id.clone(),
                team: seat,
                army: None,
            };
            made.players = (0u8..).zip(slots).map(seat).collect();
            start.setup = Arc::new(made);
        }
        if let (Some((_, scenario)), Some(map), Some(board)) = (scenario, map, &board) {
            place_relics(&mut start, &self.registry, pack, scenario, map, board)
                .map_err(|e| setup(e.to_string()))?;
        }

        let modifiers = rt.modifiers().iter().map(|m| SourcedModifier {
            source: name.clone(),
            priority: 0,
            def: m.clone(),
        });
        let los = rt.service::<Arc<dyn LineOfSight>>().cloned();
        let movement = rt.service::<Arc<dyn Movement>>().cloned();
        let engagement = rt.service::<Arc<dyn Engagement>>().cloned();
        let host = Host::new(&self.registry, pack, modifiers.collect(), board);
        let host = host.with_los(los).with_movement(movement);
        let host = host.with_engagement(engagement);
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
                Step::PlaceTokens { player, placements } => {
                    let mut put = Vec::new();
                    for TokenAt {
                        figure: handle,
                        kind,
                    } in placements
                    {
                        let found = figure(handle).map_err(at)?;
                        let unit = engine.state().figures[found.index()].unit;
                        put.push(TokenPlacement {
                            unit,
                            token: DefId(*kind),
                        });
                    }
                    host.set_rig(Vec::new());
                    let outcome = engine.submit(*player, Command::PlaceTokens(put));
                    events = settle(outcome, "place_tokens").map_err(at)?;
                    log.extend(events.iter().cloned());
                }
                Step::Activate {
                    player,
                    figure: handle,
                } => {
                    let found = figure(handle).map_err(at)?;
                    let unit = engine.state().figures[found.index()].unit;
                    host.set_rig(Vec::new());
                    let outcome = engine.submit(*player, Command::ActivateUnit(unit));
                    events = settle(outcome, "activate_unit").map_err(at)?;
                    log.extend(events.iter().cloned());
                }
                Step::EndActivation { player } => {
                    host.set_rig(Vec::new());
                    let outcome = engine.submit(*player, Command::EndActivation);
                    events = settle(outcome, "end_activation").map_err(at)?;
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
