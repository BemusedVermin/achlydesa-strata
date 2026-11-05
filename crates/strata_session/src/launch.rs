//! Starting a match from a pack set: load, resolve the ruleset, build the state and play it on
//! the simulation thread (design §13.1).

use std::path::PathBuf;
use std::sync::Arc;

use strata_lua::{LuaRules, ModuleBinding};
use strata_model::{
    ArmyList, MatchSetup, PlayerId, PlayerSetup, RuleModuleDef, RulesetDef, ScenarioDef, SeedSource,
};
use strata_modules::native_modules;
use strata_rules::{Engine, Movement, PhaseTree, RulesetSource, SourcedModifier, resolve};

use crate::controller::Controller;
use crate::names::Names;
use crate::pack_test::PackTests;
use crate::pack_test::host::{Host, find};
use crate::session::Session;
use crate::setup::{build_match, pack_identities};
use crate::sim::{SimHandle, SimLink};

/// What to play. Every seat is a human seat served by the client's channels, which suits hot-seat.
#[derive(Clone, Debug)]
pub struct LaunchSpec {
    /// Pack roots, as `strata pack validate` takes them.
    pub roots: Vec<PathBuf>,
    /// The scenario's name, possibly qualified by pack.
    pub scenario: String,
    /// The ruleset's name. `None` picks the only ruleset the packs define.
    pub ruleset: Option<String>,
    /// The seed.
    pub seed: u64,
    /// Army list of each seat in slot order; seats past the end have none.
    pub armies: Vec<Option<ArmyList>>,
    /// Stop after this many commands.
    pub commands: u32,
}

struct Rulesets<'a>(&'a PackTests, &'a str);

impl RulesetSource for Rulesets<'_> {
    fn ruleset(&self, name: &str) -> Option<&RulesetDef> {
        find::<RulesetDef>(&self.0.registry, self.1, name).map(|(_, def)| def)
    }
}

/// Starts the simulation thread. Loading happens on that thread, because the Lua states are not
/// `Send`; a failure arrives as [`crate::SimMessage::Error`].
pub fn launch(spec: LaunchSpec) -> SimHandle {
    SimHandle::spawn(move |link| {
        if let Err(message) = play(&spec, &link) {
            link.fail(crate::SessionError::Launch(message));
        }
    })
}

fn play(spec: &LaunchSpec, link: &SimLink) -> Result<(), String> {
    let loaded = PackTests::load(&spec.roots).map_err(|e| e.to_string())?;
    let tests = native_modules()
        .into_iter()
        .try_fold(loaded, PackTests::with_native)
        .map_err(|e| e.to_string())?;
    let registry = &tests.registry;
    let (scenario_id, scenario) = find::<ScenarioDef>(registry, "", &spec.scenario)
        .ok_or_else(|| format!("unknown scenario `{}`", spec.scenario))?;
    let pack = registry
        .name::<ScenarioDef>(scenario_id)
        .and_then(|n| n.split_once(':'))
        .map_or("", |(p, _)| p);
    let (ruleset_id, ruleset_name) = pick_ruleset(&tests, spec.ruleset.as_deref())?;
    let players = scenario
        .slots
        .iter()
        .take(usize::from(scenario.players.min));
    let players = players.enumerate().map(|(seat, slot)| PlayerSetup {
        id: PlayerId(seat as u8),
        slot: slot.id.clone(),
        team: seat as u8,
        army: spec.armies.get(seat).cloned().flatten(),
    });
    let setup = MatchSetup {
        packs: pack_identities(registry),
        ruleset: ruleset_id,
        format: None,
        scenario: scenario_id,
        seed: SeedSource::Plain(spec.seed),
        players: players.collect(),
    };
    let seats = setup.players.len();
    let built = build_match(registry, setup).map_err(|e| e.0)?;
    let rt = resolve(&ruleset_name, &Rulesets(&tests, pack), &tests.catalog)
        .map_err(|e| e.to_string())?;
    let source = strata_kernel::Sym::new(&ruleset_name);
    let modifiers = rt.modifiers().iter().map(|m| SourcedModifier {
        source: source.clone(),
        priority: 0,
        def: m.clone(),
    });
    let movement = rt.service::<Arc<dyn Movement>>().cloned();
    let host = Host::new(registry, pack, modifiers.collect(), Some(built.board));
    let host = host.with_movement(movement);
    let phases = PhaseTree::build(rt.phases()).map(|t| t.phases().to_vec());
    let _ = host
        .names
        .set(Arc::new(Names::of(registry, &phases.unwrap_or_default())));
    let host = Arc::new(host);
    let bindings = rt
        .configs()
        .iter()
        .filter(|(id, _)| registry.find::<RuleModuleDef>(id.as_str()).is_some())
        .map(|(id, config)| ModuleBinding {
            id: id.as_str().to_owned(),
            config: config.clone(),
        });
    let (scripts, packs) = (registry.scripts(), tests.packs.clone());
    let rules = LuaRules::new(&rt, scripts, packs, bindings.collect(), host.clone());
    let rules = Arc::new(rules);
    let _ = host.rules.set(Arc::downgrade(&rules));
    let engine = Engine::new(&rt, &*rules, built.state);
    let controllers: Vec<Box<dyn Controller>> = (0..seats)
        .map(|_| Box::new(link.human()) as Box<dyn Controller>)
        .collect();
    let mut session = Session::new(engine, controllers);
    link.run_with(&mut session, PlayerId(0), spec.commands, &*host);
    Ok(())
}

/// The ID and bare name of the ruleset called `wanted`, or of the only one there is.
fn pick_ruleset(
    tests: &PackTests,
    wanted: Option<&str>,
) -> Result<(strata_kernel::DefId, String), String> {
    let registry = &tests.registry;
    let named = |id| registry.name::<RulesetDef>(id).map(str::to_owned);
    if let Some(name) = wanted {
        let (id, _) = find::<RulesetDef>(registry, "", name)
            .ok_or_else(|| format!("unknown ruleset `{name}`"))?;
        return Ok((id, name.to_owned()));
    }
    let mut all = registry.iter::<RulesetDef>();
    match (all.next(), all.next()) {
        (Some((id, full, _)), None) => Ok((id, named(id).unwrap_or_else(|| full.to_owned()))),
        (None, _) => Err("the packs define no ruleset".into()),
        _ => Err("several rulesets are defined; name one".into()),
    }
}

#[cfg(test)]
mod tests;
