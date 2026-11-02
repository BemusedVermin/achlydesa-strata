use std::collections::BTreeMap;

use serde_json::json;
use strata_kernel::{ModuleId, SlotId, Sym, Value, WindowId};
use strata_model::{ConfigField, RulesetDef};

use crate::*;

struct Fake {
    id: &'static str,
    slot: Option<&'static str>,
    deps: Vec<ModuleId>,
    schema: ConfigSchema,
    window: Option<&'static str>,
}

impl Fake {
    fn new(id: &'static str, slot: Option<&'static str>) -> Self {
        Self {
            id,
            slot,
            deps: Vec::new(),
            schema: ConfigSchema::default(),
            window: None,
        }
    }

    fn dep(mut self, id: &str) -> Self {
        self.deps.push(ModuleId::new(id));
        self
    }

    fn window(mut self, w: &'static str) -> Self {
        self.window = Some(w);
        self
    }

    fn option(mut self, name: &str, kind: &str, default: Value) -> Self {
        let mut fields = BTreeMap::new();
        fields.insert(
            Sym::new(name),
            ConfigField {
                kind: Sym::new(kind),
                default,
            },
        );
        self.schema = ConfigSchema::new(fields);
        self
    }
}

impl RuleModule for Fake {
    fn id(&self) -> ModuleId {
        ModuleId::new(self.id)
    }
    fn slot(&self) -> Option<SlotId> {
        self.slot.map(SlotId::new)
    }
    fn depends_on(&self) -> &[ModuleId] {
        &self.deps
    }
    fn config_schema(&self) -> &ConfigSchema {
        &self.schema
    }
    fn register(&self, r: &mut Registrar, _cfg: &Value) -> Result<(), ModuleError> {
        if let Some(w) = self.window {
            r.window(WindowId::new(w));
        }
        Ok(())
    }
}

fn catalog(modules: Vec<Fake>) -> ModuleCatalog {
    let mut c = ModuleCatalog::default();
    for m in modules {
        c.register(Box::new(m)).unwrap();
    }
    c
}

fn rulesets(defs: &[(&str, serde_json::Value)]) -> BTreeMap<Sym, RulesetDef> {
    defs.iter()
        .enumerate()
        .map(|(i, (name, body))| {
            let mut body = body.clone();
            body["id"] = json!(i);
            (Sym::new(name), serde_json::from_value(body).unwrap())
        })
        .collect()
}

fn base_catalog() -> ModuleCatalog {
    catalog(vec![
        Fake::new("core:units", Some("units")).window("units.w"),
        Fake::new("core:combat", Some("combat"))
            .dep("core:units")
            .option("height_bonus", "int", Value::Int(1)),
        Fake::new("alt:combat", Some("combat")),
        Fake::new("core:tokens", Some("tokens")),
        Fake::new("morale", None),
    ])
}

fn base_rulesets() -> BTreeMap<Sym, RulesetDef> {
    rulesets(&[
        (
            "advanced",
            json!({"stats": ["life"], "dice": ["combat"], "slots": {
                "units": {"id": "core:units"},
                "combat": {"id": "core:combat"},
                "tokens": {"id": "core:tokens"},
            }}),
        ),
        (
            "intro",
            json!({"extends": "advanced", "abilities": false, "slots": {
                "tokens": null,
                "combat": {"id": "core:combat", "config": {"height_bonus": {"Int": 0}}},
            }}),
        ),
        (
            "ambush",
            json!({"extends": "intro", "slots": {"combat": {"id": "alt:combat"}},
                   "addons": [{"id": "morale"}]}),
        ),
    ])
}

fn slot_id(runtime: &RulesRuntime, slot: &str) -> Option<String> {
    runtime
        .slot(&SlotId::new(slot))
        .map(|m| m.as_str().to_owned())
}

#[test]
fn resolves_a_plain_ruleset() {
    let rt = resolve("advanced", &base_rulesets(), &base_catalog()).unwrap();
    assert_eq!(slot_id(&rt, "tokens").as_deref(), Some("core:tokens"));
    assert_eq!(rt.slot_modules().count(), 3);
    assert_eq!(rt.windows(), &[WindowId::new("units.w")]);
}

#[test]
fn extends_chain_applies_false_slots_and_overrides() {
    let rt = resolve("ambush", &base_rulesets(), &base_catalog()).unwrap();
    assert_eq!(slot_id(&rt, "tokens"), None);
    assert_eq!(slot_id(&rt, "combat").as_deref(), Some("alt:combat"));
    assert_eq!(slot_id(&rt, "units").as_deref(), Some("core:units"));
}

#[test]
fn addons_are_registered_after_slots() {
    let rt = resolve("ambush", &base_rulesets(), &base_catalog()).unwrap();
    assert_eq!(rt.addons(), &[ModuleId::new("morale")]);
}

#[test]
fn resolution_is_repeatable() {
    let order = |rt: &RulesRuntime| {
        rt.slot_modules()
            .map(|(s, m)| format!("{s}={m}"))
            .collect::<Vec<_>>()
    };
    let a = resolve("ambush", &base_rulesets(), &base_catalog()).unwrap();
    let b = resolve("ambush", &base_rulesets(), &base_catalog()).unwrap();
    assert_eq!(order(&a), order(&b));
}

#[test]
fn unknown_ruleset_and_cycles_are_errors() {
    let sets = rulesets(&[
        ("a", json!({"extends": "b"})),
        ("b", json!({"extends": "a"})),
        ("c", json!({"extends": "nope"})),
    ]);
    let cat = base_catalog();
    let err = |name| resolve(name, &sets, &cat).err().unwrap();
    assert!(matches!(err("zzz"), ResolveError::UnknownRuleset { .. }));
    assert!(matches!(err("a"), ResolveError::ExtendsCycle { .. }));
    let ResolveError::UnknownRuleset { ruleset } = err("c") else {
        panic!("expected UnknownRuleset");
    };
    assert_eq!(ruleset, "nope");
}

#[test]
fn unknown_slot_names_ruleset_and_slot() {
    let sets = rulesets(&[("r", json!({"slots": {"bogus": {"id": "core:units"}}}))]);
    let msg = resolve("r", &sets, &base_catalog())
        .err()
        .unwrap()
        .to_string();
    assert!(msg.contains("`r`") && msg.contains("`bogus`"), "{msg}");
}

#[test]
fn missing_module_names_ruleset_slot_and_module() {
    let sets = rulesets(&[("r", json!({"slots": {"combat": {"id": "no:such"}}}))]);
    let msg = resolve("r", &sets, &base_catalog())
        .err()
        .unwrap()
        .to_string();
    assert_eq!(
        msg,
        "ruleset `r`, slot `combat`, module `no:such`: module is not in the catalog"
    );
}

#[test]
fn module_in_wrong_slot_is_an_error() {
    let sets = rulesets(&[
        ("r", json!({"slots": {"units": {"id": "core:combat"}}})),
        ("s", json!({"slots": {"units": {"id": "morale"}}})),
        ("t", json!({"addons": [{"id": "core:units"}]})),
    ]);
    let cat = base_catalog();
    let err = |name| resolve(name, &sets, &cat).err().unwrap();
    assert!(matches!(err("r"), ResolveError::SlotMismatch { .. }));
    assert!(matches!(err("s"), ResolveError::SlotMismatch { .. }));
    let ResolveError::SlotMismatch { site, actual } = err("t") else {
        panic!("expected SlotMismatch");
    };
    assert_eq!((site.slot.as_str(), actual.as_str()), ("(addon)", "units"));
}

#[test]
fn dependency_on_an_empty_slot_is_an_error() {
    let sets = rulesets(&[("r", json!({"slots": {"combat": {"id": "core:combat"}}}))]);
    let err = resolve("r", &sets, &base_catalog()).err().unwrap();
    assert_eq!(
        err.to_string(),
        "ruleset `r`, slot `combat`, module `core:combat`: requires `core:units`, \
         but slot `units` is empty"
    );
}

#[test]
fn dependency_on_an_unknown_module_is_an_error() {
    let cat = catalog(vec![Fake::new("m", Some("combat")).dep("ghost")]);
    let sets = rulesets(&[("r", json!({"slots": {"combat": {"id": "m"}}}))]);
    let err = resolve("r", &sets, &cat).err().unwrap();
    assert!(matches!(err, ResolveError::MissingDependency { .. }));
    assert!(err.to_string().contains("`ghost`"));
}

#[test]
fn duplicate_addons_are_an_error() {
    let sets = rulesets(&[("r", json!({"addons": [{"id": "morale"}, {"id": "morale"}]}))]);
    let err = resolve("r", &sets, &base_catalog()).err().unwrap();
    assert!(matches!(err, ResolveError::DuplicateModule(_)));
}

fn config_error(config: serde_json::Value) -> Option<ConfigError> {
    let cat = catalog(vec![Fake::new("m", Some("combat")).option(
        "n",
        "int",
        Value::Int(2),
    )]);
    let sets = rulesets(&[(
        "r",
        json!({"slots": {"combat": {"id": "m", "config": config}}}),
    )]);
    match resolve("r", &sets, &cat) {
        Ok(_) => None,
        Err(ResolveError::Config { site, source }) => {
            assert_eq!(site.module, "m");
            Some(source)
        }
        Err(other) => panic!("unexpected {other}"),
    }
}

#[test]
fn config_is_validated_against_the_schema() {
    assert_eq!(config_error(json!({})), None);
    assert_eq!(config_error(json!({"n": {"Int": 5}})), None);
    assert!(matches!(
        config_error(json!({"n": {"Bool": true}})),
        Some(ConfigError::Type { .. })
    ));
    assert_eq!(
        config_error(json!({"x": {"Int": 1}})),
        Some(ConfigError::UnknownOption("x".into()))
    );
}

#[test]
fn schema_fills_defaults_and_requires_undefaulted_options() {
    let mut fields = BTreeMap::new();
    for (name, kind, default) in [
        ("a", "int", Value::Int(7)),
        ("b", "str", Value::Nil),
        ("c", "wat", Value::Int(0)),
    ] {
        fields.insert(
            Sym::new(name),
            ConfigField {
                kind: Sym::new(kind),
                default,
            },
        );
    }
    let schema = ConfigSchema::new(fields.clone());
    assert_eq!(
        schema.resolve(&BTreeMap::new()),
        Err(ConfigError::MissingOption("b".into()))
    );
    let given = BTreeMap::from([(Sym::new("b"), Value::from("x"))]);
    assert!(matches!(
        schema.resolve(&given),
        Err(ConfigError::UnknownType { .. })
    ));
    fields.remove("c");
    let cfg = ConfigSchema::new(fields).resolve(&given).unwrap();
    assert_eq!(cfg.get("a"), Ok(&Value::Int(7)));
    assert_eq!(cfg.get("b"), Ok(&Value::from("x")));
}

#[test]
fn module_errors_and_conflicts_name_the_module() {
    struct Failing;
    impl RuleModule for Failing {
        fn id(&self) -> ModuleId {
            ModuleId::new("bad")
        }
        fn slot(&self) -> Option<SlotId> {
            None
        }
        fn depends_on(&self) -> &[ModuleId] {
            &[]
        }
        fn config_schema(&self) -> &ConfigSchema {
            static EMPTY: std::sync::OnceLock<ConfigSchema> = std::sync::OnceLock::new();
            EMPTY.get_or_init(ConfigSchema::default)
        }
        fn register(&self, _: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
            Err(ModuleError::Invalid("boom".into()))
        }
    }
    let mut cat = ModuleCatalog::default();
    cat.register(Box::new(Failing)).unwrap();
    let sets = rulesets(&[("r", json!({"addons": [{"id": "bad"}]}))]);
    let err = resolve("r", &sets, &cat).err().unwrap();
    assert_eq!(
        err.to_string(),
        "ruleset `r`, slot `(addon)`, module `bad`: boom"
    );

    let cat = catalog(vec![
        Fake::new("w1", None).window("w"),
        Fake::new("w2", None).window("w"),
    ]);
    let sets = rulesets(&[("r", json!({"addons": [{"id": "w1"}, {"id": "w2"}]}))]);
    let err = resolve("r", &sets, &cat).err().unwrap();
    assert!(matches!(err, ResolveError::Conflict { .. }));
    assert!(err.to_string().contains("`w2`") && err.to_string().contains("`w`"));
}

#[test]
fn duplicate_catalog_entries_are_rejected() {
    let mut cat = base_catalog();
    let err = cat.register(Box::new(Fake::new("morale", None))).err();
    assert_eq!(err, Some(DuplicateModule(ModuleId::new("morale"))));
}

trait Los: Send + Sync {
    fn name(&self) -> &str;
}
struct Geometric;
impl Los for Geometric {
    fn name(&self) -> &str {
        "geometric"
    }
}

#[test]
fn services_and_lua_apis_are_reachable_from_the_runtime() {
    struct Provider;
    impl RuleModule for Provider {
        fn id(&self) -> ModuleId {
            ModuleId::new("p")
        }
        fn slot(&self) -> Option<SlotId> {
            Some(SlotId::new("los"))
        }
        fn depends_on(&self) -> &[ModuleId] {
            &[]
        }
        fn config_schema(&self) -> &ConfigSchema {
            static EMPTY: std::sync::OnceLock<ConfigSchema> = std::sync::OnceLock::new();
            EMPTY.get_or_init(ConfigSchema::default)
        }
        fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
            r.service::<dyn Los>(Box::new(Geometric));
            r.lua_api(
                "los",
                LuaApi {
                    functions: vec![Sym::new("visible")],
                },
            );
            r.phase(
                PhaseDef {
                    id: Sym::new("morale"),
                },
                Anchor::After(Sym::new("turns.round_end")),
            );
            Ok(())
        }
    }
    let mut cat = ModuleCatalog::default();
    cat.register(Box::new(Provider)).unwrap();
    let sets = rulesets(&[("r", json!({"slots": {"los": {"id": "p"}}}))]);
    let rt = resolve("r", &sets, &cat).unwrap();
    assert_eq!(rt.service::<dyn Los>().unwrap().name(), "geometric");
    assert!(rt.service::<Geometric>().is_none());
    assert_eq!(rt.lua_api("los").unwrap().functions.len(), 1);
    assert_eq!(rt.phases().len(), 1);
}

#[test]
fn standard_slots_match_the_design() {
    assert_eq!(slots::STANDARD.len(), 12);
    assert!(slots::is_standard(slots::VICTORY));
    assert!(!slots::is_standard("bogus"));
}

#[test]
fn runtime_is_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<RulesRuntime>();
}
