//! Turning descriptor trees into typed nodes.

use std::collections::BTreeMap;

use strata_kernel::{Sym, Value};
use strata_model::{AbilityDef, Descriptor, Logic};

use super::{AttackKind, Base, Cmp, Cond, Eff, Node, Sel, Val};

/// A descriptor is not part of the vocabulary or is malformed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DescriptorError {
    /// The tag names no descriptor.
    #[error("unknown descriptor `{namespace}.{name}`")]
    Unknown {
        /// The namespace, such as `sel`.
        namespace: &'static str,
        /// The unknown name.
        name: String,
    },
    /// The descriptor has a field it does not understand.
    #[error("`{tag}` has no field `{field}`")]
    UnknownField {
        /// The descriptor.
        tag: String,
        /// The field.
        field: String,
    },
    /// A required field is absent.
    #[error("`{tag}` needs `{field}`")]
    Missing {
        /// The descriptor.
        tag: String,
        /// The field.
        field: &'static str,
    },
    /// A field or the descriptor itself has the wrong shape.
    #[error("{0}")]
    Invalid(String),
}

fn invalid<T>(msg: impl Into<String>) -> Result<T, DescriptorError> {
    Err(DescriptorError::Invalid(msg.into()))
}

/// Compiles a descriptor into a typed node.
pub fn compile(descriptor: &Descriptor) -> Result<Node, DescriptorError> {
    let value = &descriptor.0;
    match value {
        Value::Bool(_) => cond(value).map(Node::Cond),
        Value::Int(_) => val(value).map(Node::Val),
        Value::Str(_) => sel(value).map(Node::Sel),
        Value::Map(m) if m.contains_key("sel") => sel(value).map(Node::Sel),
        Value::Map(m) if m.contains_key("cond") => cond(value).map(Node::Cond),
        Value::Map(m) if m.contains_key("val") => val(value).map(Node::Val),
        Value::Map(m) if m.contains_key("eff") => eff(value).map(Node::Eff),
        other => invalid(format!("a {} is not a descriptor", other.type_name())),
    }
}

/// The effect a logic names, if it is a native `eff` descriptor; `None` for a script or any other
/// descriptor, such as `{ native = "<name>" }`.
pub fn native_effect(logic: &Logic) -> Option<Result<Eff, DescriptorError>> {
    match logic {
        Logic::Native(Descriptor(value @ Value::Map(m))) if m.contains_key("eff") => {
            Some(eff(value))
        }
        _ => None,
    }
}

/// A descriptor of an ability that does not compile, and where it sits.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{path}: {error}")]
pub struct LogicFault {
    /// The route to the logic field inside the ability, such as `modifiers/1/when`.
    pub path: String,
    /// What is wrong with it.
    pub error: DescriptorError,
}

/// `logic` at `field` of entry `i` of `list`, with its route and role.
fn entry<'a>(
    list: &str,
    i: usize,
    field: &str,
    logic: Option<&'a Logic>,
) -> Option<(String, FieldRole, &'a Logic)> {
    let role = FieldRole::of(list, field);
    logic.map(|logic| (format!("{list}/{}/{field}", i + 1), role, logic))
}

/// What a logic field does, which decides what it may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldRole {
    /// It is asked a question, as a `filter`, `when`, `select`, `targets` or `value`: any native
    /// descriptor that compiles, or a script.
    Asks,
    /// It acts, as a reaction's `effect`, a replacement's `replace` or a relic's `effect`: an
    /// `eff` descriptor or a script.
    Acts,
    /// It acts where Rust logic can also run, as a trigger's `effect` or an action's `resolve`:
    /// an `eff` descriptor, a script, or `{ native = "<name>" }`, which the resolver checks.
    ActsOrNamesRust,
}

impl FieldRole {
    /// The role of `field` of an entry of the hook list `list` (such as `triggers`), named as in
    /// an ability or a module registration (`trigger`).
    pub fn of(list: &str, field: &str) -> Self {
        match (list.trim_end_matches('s'), field) {
            ("trigger", "effect") | ("action", "resolve") => FieldRole::ActsOrNamesRust,
            (_, "effect" | "replace" | "resolve") => FieldRole::Acts,
            _ => FieldRole::Asks,
        }
    }
}

/// Compiles every native logic of an ability, so unknown descriptors fail at load. A
/// `{ native = "<name>" }` effect or resolver names Rust logic, which the resolver checks, so it is
/// skipped here; it is refused where Rust logic cannot run.
pub fn check_ability(ability: &AbilityDef) -> Result<(), LogicFault> {
    let mut logics: Vec<(String, FieldRole, &Logic)> = Vec::new();
    for (i, m) in ability.modifiers.iter().enumerate() {
        logics.extend(entry("modifiers", i, "select", m.select.as_ref()));
        logics.extend(entry("modifiers", i, "when", m.when.as_ref()));
        logics.extend(entry("modifiers", i, "value", Some(&m.value)));
    }
    for (i, a) in ability.actions.iter().enumerate() {
        logics.extend(entry("actions", i, "targets", a.targets.as_ref()));
        logics.extend(entry("actions", i, "resolve", Some(&a.resolve)));
    }
    for (i, t) in ability.triggers.iter().enumerate() {
        logics.extend(entry("triggers", i, "filter", t.filter.as_ref()));
        logics.extend(entry("triggers", i, "effect", Some(&t.effect)));
    }
    for (i, r) in ability.replacements.iter().enumerate() {
        logics.extend(entry("replacements", i, "filter", r.filter.as_ref()));
        logics.extend(entry("replacements", i, "replace", Some(&r.replace)));
    }
    for (i, r) in ability.reactions.iter().enumerate() {
        logics.extend(entry("reactions", i, "when", r.when.as_ref()));
        logics.extend(entry("reactions", i, "effect", Some(&r.effect)));
    }
    for (path, role, logic) in logics {
        check_logic(logic, role).map_err(|error| LogicFault { path, error })?;
    }
    Ok(())
}

/// Checks one logic field for its role. A script always passes.
pub fn check_logic(logic: &Logic, role: FieldRole) -> Result<(), DescriptorError> {
    match logic {
        _ if role != FieldRole::Asks => check_effect(logic, role == FieldRole::ActsOrNamesRust),
        Logic::Native(d) => compile(d).map(drop),
        Logic::Script(_) => Ok(()),
    }
}

/// Checks a logic that acts, such as a trigger's `effect`: a script, an `eff` descriptor, or,
/// where `rust` logic can run, a `{ native = "<name>" }` name, which the resolver checks.
pub fn check_effect(logic: &Logic, rust: bool) -> Result<(), DescriptorError> {
    if crate::native_name(logic).is_some() {
        return match rust {
            true => Ok(()),
            false => invalid(
                "`{ native = \"<name>\" }` runs only as a trigger's `effect` or an action's \
                 `resolve`; here, use an `eff` descriptor or a function",
            ),
        };
    }
    match (logic, native_effect(logic)) {
        (Logic::Script(_), _) => Ok(()),
        (_, Some(compiled)) => compiled.map(drop),
        (_, None) => invalid(match rust {
            true => {
                "an effect must be an `eff` descriptor, `{ native = \"<name>\" }` or a function"
            }
            false => "an effect must be an `eff` descriptor or a function",
        }),
    }
}

/// The fields of one descriptor, handed out once each.
struct Args<'a> {
    tag: String,
    fields: BTreeMap<&'a str, &'a Value>,
}

impl<'a> Args<'a> {
    fn new(namespace: &'static str, value: &'a Value) -> Result<(Self, &'a str), DescriptorError> {
        let Value::Map(map) = value else {
            return invalid(format!(
                "a {} is not a `{namespace}` descriptor",
                value.type_name()
            ));
        };
        let Some(Value::Str(name)) = map.get(namespace) else {
            return invalid(format!("`{namespace}` must name a descriptor"));
        };
        let fields = map
            .iter()
            .filter(|(k, _)| k.as_str() != namespace)
            .map(|(k, v)| (k.as_str(), v))
            .collect();
        let tag = format!("{namespace}.{}", name.as_str());
        Ok((Self { tag, fields }, name.as_str()))
    }

    fn opt(&mut self, key: &str) -> Option<&'a Value> {
        self.fields.remove(key)
    }

    fn req(&mut self, key: &'static str) -> Result<&'a Value, DescriptorError> {
        let tag = self.tag.clone();
        self.opt(key)
            .ok_or(DescriptorError::Missing { tag, field: key })
    }

    fn int(&mut self, key: &'static str) -> Result<Option<i64>, DescriptorError> {
        match self.opt(key) {
            None => Ok(None),
            Some(Value::Int(n)) => Ok(Some(*n)),
            Some(_) => invalid(format!("`{}.{key}` must be an integer", self.tag)),
        }
    }

    fn opt_sym(&mut self, key: &'static str) -> Result<Option<Sym>, DescriptorError> {
        match self.opt(key) {
            None => Ok(None),
            Some(Value::Str(s)) => Ok(Some(s.clone())),
            Some(_) => invalid(format!("`{}.{key}` must be a string", self.tag)),
        }
    }

    fn sym(&mut self, key: &'static str) -> Result<Sym, DescriptorError> {
        let tag = self.tag.clone();
        self.opt_sym(key)?
            .ok_or(DescriptorError::Missing { tag, field: key })
    }

    fn list<T>(
        &mut self,
        key: &'static str,
        f: fn(&Value) -> Result<T, DescriptorError>,
    ) -> Result<Vec<T>, DescriptorError> {
        match self.req(key)? {
            Value::List(items) if !items.is_empty() => items.iter().map(f).collect(),
            _ => invalid(format!("`{}.{key}` must be a non-empty list", self.tag)),
        }
    }

    fn bool(&mut self, key: &'static str) -> Result<Option<bool>, DescriptorError> {
        match self.opt(key) {
            None => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(_) => invalid(format!("`{}.{key}` must be a boolean", self.tag)),
        }
    }

    /// The value `key`, or `default` when it is absent.
    fn val_or(&mut self, key: &str, default: i64) -> Result<Val, DescriptorError> {
        self.opt(key).map_or(Ok(Val::Lit(default)), val)
    }

    /// The selector `key`, or `target` when it is absent.
    fn sel_or_target(&mut self, key: &str) -> Result<Sel, DescriptorError> {
        self.opt(key).map_or(Ok(Sel::target()), sel)
    }

    fn sel(&mut self, key: &'static str) -> Result<Sel, DescriptorError> {
        sel(self.req(key)?)
    }

    fn sel_or_self(&mut self, key: &str) -> Result<Sel, DescriptorError> {
        self.opt(key).map_or(Ok(Sel::own()), sel)
    }

    fn done(self) -> Result<(), DescriptorError> {
        match self.fields.keys().next() {
            Some(field) => Err(DescriptorError::UnknownField {
                tag: self.tag.clone(),
                field: (*field).to_owned(),
            }),
            None => Ok(()),
        }
    }
}

impl Sel {
    /// The selector `self`.
    pub fn own() -> Self {
        Sel {
            base: Base::Own,
            trait_value: None,
            kind: None,
            adjacent_to: None,
            within: None,
            of: None,
            in_los: None,
            engaged: None,
        }
    }

    /// The selector `target`.
    pub fn target() -> Self {
        Sel {
            base: Base::Target,
            ..Sel::own()
        }
    }
}

fn unknown<T>(namespace: &'static str, name: &str) -> Result<T, DescriptorError> {
    Err(DescriptorError::Unknown {
        namespace,
        name: name.to_owned(),
    })
}

fn sel(value: &Value) -> Result<Sel, DescriptorError> {
    let (mut args, name) = match value {
        Value::Str(name) => {
            let tag = format!("sel.{}", name.as_str());
            let bare = Args {
                tag,
                fields: BTreeMap::new(),
            };
            (bare, name.as_str())
        }
        _ => Args::new("sel", value)?,
    };
    let base = match name {
        "self" => Base::Own,
        "allies" => Base::Allies,
        "enemies" => Base::Enemies,
        "my_units" => Base::MyUnits,
        "occupant" => Base::Occupant,
        "target" => Base::Target,
        other => return unknown("sel", other),
    };
    let within = match args.int("within")? {
        Some(n) => Some(u32::try_from(n).or_else(|_| invalid("`within` must not be negative"))?),
        None => None,
    };
    let out = Sel {
        base,
        trait_value: args.opt_sym("trait")?,
        kind: args.opt_sym("kind")?,
        adjacent_to: args.opt("adjacent_to").map(sel).transpose()?.map(Box::new),
        within,
        of: args.opt("of").map(sel).transpose()?.map(Box::new),
        in_los: args.opt("in_los").map(sel).transpose()?.map(Box::new),
        engaged: args.bool("engaged")?,
    };
    if out.of.is_some() && within.is_none() {
        return invalid("`of` only goes with `within`");
    }
    args.done()?;
    Ok(out)
}

fn cond(value: &Value) -> Result<Cond, DescriptorError> {
    if let Value::Bool(b) = value {
        return Ok(Cond::Const(*b));
    }
    let (mut args, name) = Args::new("cond", value)?;
    let out = match name {
        "all" => Cond::All(args.list("of", cond)?),
        "any" => Cond::Any(args.list("of", cond)?),
        "none" => Cond::None(args.list("of", cond)?),
        "not" => Cond::Not(Box::new(cond(args.req("of")?)?)),
        "eq" | "ne" | "lt" | "lte" | "gt" | "gte" => {
            let op = match name {
                "eq" => Cmp::Eq,
                "ne" => Cmp::Ne,
                "lt" => Cmp::Lt,
                "lte" => Cmp::Lte,
                "gt" => Cmp::Gt,
                _ => Cmp::Gte,
            };
            let (a, b) = (val(args.req("a")?)?, val(args.req("b")?)?);
            Cond::Compare(op, Box::new(a), Box::new(b))
        }
        "count" => Cond::Count {
            of: args.sel("of")?,
            min: args.int("min")?.unwrap_or(1),
            max: args.int("max")?,
        },
        "phase" => Cond::Phase(args.sym("is")?),
        "on_terrain" => Cond::OnTerrain {
            terrain: args.sym("terrain")?,
            who: args.sel_or_self("who")?,
        },
        "higher_than" => Cond::HigherThan {
            than: args.sel("than")?,
            by: args.int("by")?.unwrap_or(1),
        },
        "engaged" => Cond::Engaged(args.sel_or_self("who")?),
        "in_los" => Cond::InLos {
            from: args.sel_or_self("from")?,
            to: args.sel("to")?,
        },
        "attack_kind" => Cond::AttackKind(match args.sym("is")?.as_str() {
            "melee" => AttackKind::Melee,
            "ranged" => AttackKind::Ranged,
            other => {
                return invalid(format!(
                    "`cond.attack_kind.is` is melee or ranged, not `{other}`"
                ));
            }
        }),
        other => return unknown("cond", other),
    };
    args.done()?;
    Ok(out)
}

fn val(value: &Value) -> Result<Val, DescriptorError> {
    if let Value::Int(n) = value {
        return Ok(Val::Lit(*n));
    }
    let (mut args, name) = Args::new("val", value)?;
    let out = match name {
        "stat" => Val::Stat {
            stat: args.sym("stat")?,
            of: args.sel_or_self("of")?,
        },
        "count" => Val::Count(args.sel("of")?),
        "distance" => Val::Distance(args.sel("to")?),
        "level_difference" => Val::LevelDifference(args.sel("to")?),
        "sum" => Val::Sum(args.list("of", val)?),
        "min" => Val::Min(args.list("of", val)?),
        "max" => Val::Max(args.list("of", val)?),
        other => return unknown("val", other),
    };
    args.done()?;
    Ok(out)
}

fn eff(value: &Value) -> Result<Eff, DescriptorError> {
    let (mut args, name) = Args::new("eff", value)?;
    let out = match name {
        "wound" => Eff::Wound {
            who: args.sel_or_target("who")?,
            amount: args.val_or("amount", 1)?,
        },
        "heal" => Eff::Heal {
            who: args.sel_or_target("who")?,
            amount: args.val_or("amount", 1)?,
        },
        "place" => Eff::Place {
            who: args.sel_or_target("who")?,
            within: val(args.req("within")?)?,
            of: args.sel_or_self("of")?,
        },
        "remove" => Eff::Remove {
            who: args.sel_or_target("who")?,
        },
        "move" => Eff::Move {
            who: args.sel_or_target("who")?,
            up_to: val(args.req("up_to")?)?,
        },
        "grant_slot" => Eff::GrantSlot {
            slot: args.sym("slot")?,
            count: args.val_or("count", 1)?,
            who: args.sel_or_self("who")?,
        },
        "add_marker" => Eff::AddMarker {
            marker: args.sym("marker")?,
            count: args.val_or("count", 1)?,
            rounds: match args.int("rounds")? {
                None => None,
                Some(n) if n >= 1 => Some(u32::try_from(n).unwrap_or(u32::MAX)),
                Some(_) => return invalid("`eff.add_marker.rounds` must be at least 1"),
            },
            who: args.sel_or_target("who")?,
        },
        "take_control" => Eff::TakeControl {
            who: args.sel_or_target("who")?,
        },
        "cancel_wounds" => Eff::CancelWounds,
        "excess_blocks_wound_attacker" => Eff::ExcessBlocksWoundAttacker,
        other => return unknown("eff", other),
    };
    args.done()?;
    Ok(out)
}
