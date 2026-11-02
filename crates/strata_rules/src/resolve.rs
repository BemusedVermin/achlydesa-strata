//! Ruleset resolution (design §6.3).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use strata_kernel::{ModuleId, SlotId, Sym};
use strata_model::{ModuleRef, RulesetDef};

use crate::catalog::ModuleCatalog;
use crate::config::ConfigError;
use crate::module::{ModuleError, RuleModule};
use crate::registrar::Registrar;
use crate::runtime::RulesRuntime;
use crate::slots;

/// Where a resolution error arose: a ruleset, a slot (or `(addon)`) and a module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    /// The ruleset being resolved.
    pub ruleset: String,
    /// The slot the module was named for, or `(addon)`.
    pub slot: String,
    /// The module involved.
    pub module: String,
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ruleset `{}`, slot `{}`, module `{}`",
            self.ruleset, self.slot, self.module
        )
    }
}

/// A ruleset could not be turned into a runtime.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// A ruleset, or one it extends, does not exist.
    #[error("ruleset `{ruleset}` does not exist")]
    UnknownRuleset {
        /// The missing ruleset.
        ruleset: String,
    },
    /// The `extends` chain loops back on itself.
    #[error("ruleset `{ruleset}` extends itself through `{base}`")]
    ExtendsCycle {
        /// The ruleset being resolved.
        ruleset: String,
        /// The ruleset that closes the loop.
        base: String,
    },
    /// A ruleset names a slot that is not one of the twelve standard slots.
    #[error("ruleset `{ruleset}` sets unknown slot `{slot}`")]
    UnknownSlot {
        /// The ruleset that sets the slot.
        ruleset: String,
        /// The unknown slot.
        slot: String,
    },
    /// The catalog has no such module.
    #[error("{0}: module is not in the catalog")]
    MissingModule(Box<Site>),
    /// The same module is active twice.
    #[error("{0}: module is active more than once")]
    DuplicateModule(Box<Site>),
    /// A module sits in a slot it does not fill.
    #[error("{site}: module fills {actual}")]
    SlotMismatch {
        /// Where the module was named.
        site: Box<Site>,
        /// The slot the module actually fills, or `(addon)` for an add-on.
        actual: String,
    },
    /// A dependency is not active.
    #[error("{site}: requires `{dependency}`, which is not active")]
    MissingDependency {
        /// The module with the dependency.
        site: Box<Site>,
        /// The required module.
        dependency: String,
    },
    /// A dependency would fill a slot the ruleset leaves empty.
    #[error("{site}: requires `{dependency}`, but slot `{empty}` is empty")]
    EmptySlotDependency {
        /// The module with the dependency.
        site: Box<Site>,
        /// The required module.
        dependency: String,
        /// The empty slot the dependency belongs in.
        empty: String,
    },
    /// The module's config does not match its schema.
    #[error("{site}: {source}")]
    Config {
        /// The module.
        site: Box<Site>,
        /// The schema violation.
        source: ConfigError,
    },
    /// The module failed while registering.
    #[error("{site}: {source}")]
    Module {
        /// The module.
        site: Box<Site>,
        /// The module's error.
        source: ModuleError,
    },
    /// The module registered something another module already provides.
    #[error("{site}: {what}")]
    Conflict {
        /// The module.
        site: Box<Site>,
        /// What clashed.
        what: String,
    },
}

/// Finds rulesets by name, such as `advanced` or `strata_std:advanced`.
pub trait RulesetSource {
    /// Returns the named ruleset.
    fn ruleset(&self, name: &str) -> Option<&RulesetDef>;
}

impl RulesetSource for BTreeMap<Sym, RulesetDef> {
    fn ruleset(&self, name: &str) -> Option<&RulesetDef> {
        self.get(name)
    }
}

/// One active module and where it came from.
struct Entry<'a> {
    site: Box<Site>,
    mref: &'a ModuleRef,
    module: &'a dyn RuleModule,
    slot: Option<SlotId>,
}

/// Resolves the ruleset `name` into a finished [`RulesRuntime`].
///
/// Follows `extends`, applies slot overrides (`None` empties a slot), adds add-ons, checks
/// dependencies and config, then runs each module's `register` in slot order followed by add-ons.
pub fn resolve(
    name: &str,
    source: &impl RulesetSource,
    catalog: &ModuleCatalog,
) -> Result<RulesRuntime, ResolveError> {
    let chain = extends_chain(name, source)?;
    let mut slot_refs: BTreeMap<SlotId, Option<&ModuleRef>> = BTreeMap::new();
    let mut addon_refs: Vec<&ModuleRef> = Vec::new();
    for (owner, def) in &chain {
        for (slot, mref) in &def.slots {
            if !slots::is_standard(slot.as_str()) {
                return Err(ResolveError::UnknownSlot {
                    ruleset: owner.clone(),
                    slot: slot.to_string(),
                });
            }
            slot_refs.insert(slot.clone(), mref.as_ref());
        }
        addon_refs.extend(&def.addons);
    }

    let named = slot_refs
        .iter()
        .filter_map(|(slot, m)| m.map(|m| (Some(slot.clone()), m)))
        .chain(addon_refs.into_iter().map(|m| (None, m)));
    let mut entries = Vec::new();
    let mut active = BTreeSet::new();
    for (slot, mref) in named {
        let site = Box::new(Site {
            ruleset: name.to_owned(),
            slot: slot
                .as_ref()
                .map_or("(addon)".to_owned(), SlotId::to_string),
            module: mref.id.to_string(),
        });
        let module = catalog
            .get(&mref.id)
            .ok_or_else(|| ResolveError::MissingModule(site.clone()))?;
        if !active.insert(mref.id.clone()) {
            return Err(ResolveError::DuplicateModule(site));
        }
        let actual = module.slot();
        if actual != slot {
            let actual = actual.map_or("(addon)".to_owned(), |s| s.to_string());
            return Err(ResolveError::SlotMismatch { site, actual });
        }
        entries.push(Entry {
            site,
            mref,
            module,
            slot,
        });
    }

    for e in &entries {
        check_dependencies(e, &active, &slot_refs, catalog)?;
    }

    let mut reg = Registrar::default();
    for e in &entries {
        let cfg = e
            .module
            .config_schema()
            .resolve(&e.mref.config)
            .map_err(|source| ResolveError::Config {
                site: e.site.clone(),
                source,
            })?;
        e.module
            .register(&mut reg, &cfg)
            .map_err(|source| ResolveError::Module {
                site: e.site.clone(),
                source,
            })?;
        if let Some(what) = reg.take_conflict() {
            return Err(ResolveError::Conflict {
                site: e.site.clone(),
                what,
            });
        }
    }

    Ok(RulesRuntime {
        slots: entries
            .iter()
            .filter_map(|e| Some((e.slot.clone()?, e.mref.id.clone())))
            .collect(),
        addons: entries
            .iter()
            .filter(|e| e.slot.is_none())
            .map(|e| e.mref.id.clone())
            .collect(),
        reg,
    })
}

/// Returns the ruleset and everything it extends, base first, each with its name.
fn extends_chain<'a>(
    name: &str,
    source: &'a impl RulesetSource,
) -> Result<Vec<(String, &'a RulesetDef)>, ResolveError> {
    let mut chain = Vec::new();
    let mut seen = BTreeSet::new();
    let mut next = Some(name.to_owned());
    while let Some(current) = next {
        if !seen.insert(current.clone()) {
            return Err(ResolveError::ExtendsCycle {
                ruleset: name.to_owned(),
                base: current,
            });
        }
        let def = source
            .ruleset(&current)
            .ok_or_else(|| ResolveError::UnknownRuleset {
                ruleset: current.clone(),
            })?;
        next = def.extends.as_ref().map(ToString::to_string);
        chain.push((current, def));
    }
    chain.reverse();
    Ok(chain)
}

fn check_dependencies(
    e: &Entry<'_>,
    active: &BTreeSet<ModuleId>,
    slot_refs: &BTreeMap<SlotId, Option<&ModuleRef>>,
    catalog: &ModuleCatalog,
) -> Result<(), ResolveError> {
    let missing = e.module.depends_on().iter().find(|d| !active.contains(*d));
    if let Some(dep) = missing {
        let dependency = dep.to_string();
        let empty = catalog
            .get(dep)
            .and_then(|m| m.slot())
            .filter(|s| matches!(slot_refs.get(s), None | Some(None)));
        return Err(match empty {
            Some(empty) => ResolveError::EmptySlotDependency {
                site: e.site.clone(),
                dependency,
                empty: empty.to_string(),
            },
            None => ResolveError::MissingDependency {
                site: e.site.clone(),
                dependency,
            },
        });
    }
    Ok(())
}
