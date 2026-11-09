//! A built match with its rules: everything an engine needs to play a setup.

use std::sync::Arc;

use strata_kernel::Sym;
use strata_lua::{LuaRules, ModuleBinding};
use strata_model::{GameState, MatchSetup, RuleModuleDef, RulesetDef, ScenarioDef};
use strata_rules::{
    Engagement, Engine, LineOfSight, Movement, RulesRuntime, SourcedModifier, resolve,
};

use super::PackTests;
use super::host::Host;
use super::run::Rulesets;
use crate::setup::{SetupError, build_match};

/// The rules of a setup's ruleset, running against the match it builds.
pub struct MatchRig {
    rt: RulesRuntime,
    rules: Arc<LuaRules>,
    start: GameState,
}

impl MatchRig {
    /// Builds the match `setup` describes and the rules it plays under.
    pub fn new(tests: &PackTests, setup: MatchSetup) -> Result<Self, SetupError> {
        let registry = &tests.registry;
        let err = |m: String| SetupError(m);
        let scenario = registry.name::<ScenarioDef>(setup.scenario);
        let pack = scenario
            .and_then(|n| n.split_once(':'))
            .map_or("", |(p, _)| p);
        let name = registry
            .name::<RulesetDef>(setup.ruleset)
            .ok_or_else(|| err("the ruleset is not in the registry".into()))?
            .to_owned();
        let built = build_match(registry, setup)?;
        let rulesets = Rulesets { tests, pack };
        let rt = resolve(&name, &rulesets, &tests.catalog).map_err(|e| err(e.to_string()))?;
        let modifiers = rt.modifiers().iter().map(|m| SourcedModifier {
            source: Sym::new(&name),
            priority: 0,
            def: m.clone(),
        });
        let los = rt.service::<Arc<dyn LineOfSight>>().cloned();
        let host = Host::new(registry, pack, modifiers.collect(), Some(built.board));
        let movement = rt.service::<Arc<dyn Movement>>().cloned();
        let engagement = rt.service::<Arc<dyn Engagement>>().cloned();
        let host = host.with_los(los).with_movement(movement);
        let host = host.with_pair_queries(rt.pair_queries());
        let host = Arc::new(host.with_engagement(engagement));
        let lua = rt
            .configs()
            .iter()
            .filter(|(id, _)| registry.find::<RuleModuleDef>(id.as_str()).is_some())
            .map(|(id, config)| ModuleBinding {
                id: id.as_str().to_owned(),
                config: config.clone(),
            });
        let (scripts, packs) = (registry.scripts(), tests.packs.clone());
        let rules = LuaRules::new(&rt, scripts, packs, lua.collect(), host.clone());
        let rules = Arc::new(rules);
        let _ = host.rules.set(Arc::downgrade(&rules));
        Ok(Self {
            rt,
            rules,
            start: built.state,
        })
    }

    /// A fresh engine at the start of the match.
    pub fn engine(&self) -> Engine<'_> {
        Engine::new(&self.rt, &*self.rules, self.start.clone())
    }

    /// The resolved rules, which name the actions a command refers to.
    pub fn runtime(&self) -> &RulesRuntime {
        &self.rt
    }
}
