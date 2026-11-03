//! Running one test: resolve the ruleset, place the figures, then play the steps.

use std::sync::Arc;

use strata_lua::{LuaRules, ModuleBinding};
use strata_model::{Command, Event, MapDef, RuleModuleDef, RulesetDef, Step, TestDef};
use strata_rules::{Engine, Outcome, RulesetSource, SourcedModifier, event_kind, resolve};

use super::host::{Host, find};
use super::report::Failure;
use super::{PackTests, state};

/// Finds a test's rulesets by name, preferring its own pack.
struct Rulesets<'a> {
    tests: &'a PackTests,
    pack: &'a str,
}

impl RulesetSource for Rulesets<'_> {
    fn ruleset(&self, name: &str) -> Option<&RulesetDef> {
        let found = find::<RulesetDef>(&self.tests.registry, self.pack, name);
        found.map(|(_, def)| def)
    }
}

impl PackTests {
    /// Runs the test `id`, stopping at the first step that fails.
    pub(super) fn run_one(&self, id: &str, def: &TestDef) -> Result<(), Failure> {
        let setup = |message: String| Failure {
            step: None,
            message,
        };
        let pack = id.split_once(':').map_or("", |(pack, _)| pack);
        let name = def
            .ruleset
            .as_ref()
            .ok_or_else(|| setup("the test names no ruleset".into()))?;
        if let Some(map) = &def.map {
            let found = find::<MapDef>(&self.registry, pack, map.as_str());
            found.ok_or_else(|| setup(format!("unknown map `{map}`")))?;
        }
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
        let host = Arc::new(Host::new(&self.registry, pack, modifiers.collect()));
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
            match step {
                Step::Custom {
                    player,
                    id,
                    payload,
                } => {
                    let command = Command::Custom {
                        id: id.clone(),
                        payload: payload.clone(),
                    };
                    match engine.submit(*player, command) {
                        Outcome::Committed(emitted) => events = emitted,
                        Outcome::NeedsDecision { decision, .. } => {
                            return Err(at(format!(
                                "`{id}` asked an unanswered {:?}",
                                decision.kind
                            )));
                        }
                        Outcome::Rejected(violation) => {
                            return Err(at(format!("`{id}` was rejected: {violation}")));
                        }
                    }
                }
                Step::ExpectStat {
                    figure,
                    stat,
                    value,
                } => {
                    let found =
                        state::figure(engine.state(), &self.registry, figure).map_err(at)?;
                    let got = host.stat(engine.state(), found, stat.as_str());
                    let got = got.map_err(|e| at(e.to_string()))?;
                    if got != *value {
                        return Err(at(format!("{figure} {stat}: expected {value}, got {got}")));
                    }
                }
                Step::ExpectEvent { kind } => {
                    if !events.iter().any(|e| event_kind(e) == kind.as_str()) {
                        let got: Vec<&str> = events.iter().map(event_kind).collect();
                        return Err(at(format!("event `{kind}`: expected it, got {got:?}")));
                    }
                }
            }
        }
        Ok(())
    }
}
