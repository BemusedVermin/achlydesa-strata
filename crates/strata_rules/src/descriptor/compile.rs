//! Turning descriptor trees into typed nodes.

use std::collections::BTreeMap;

use strata_kernel::{Sym, Value};
use strata_model::{AbilityDef, Descriptor, Logic};

use super::{Base, Cmp, Cond, Node, Sel, Val};

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
        other => invalid(format!("a {} is not a descriptor", other.type_name())),
    }
}

/// Compiles every native logic of an ability, so unknown descriptors fail at load.
pub fn check_ability(ability: &AbilityDef) -> Result<(), DescriptorError> {
    let modifiers = ability.modifiers.iter();
    let logics = modifiers.flat_map(|m| [m.select.as_ref(), m.when.as_ref(), Some(&m.value)]);
    let actions = ability.actions.iter().map(|a| a.targets.as_ref());
    let triggers = ability.triggers.iter().map(|t| t.filter.as_ref());
    let replacements = ability.replacements.iter().map(|r| r.filter.as_ref());
    let reactions = ability.reactions.iter().map(|r| r.when.as_ref());
    for logic in logics
        .chain(actions)
        .chain(triggers)
        .chain(replacements)
        .chain(reactions)
        .flatten()
    {
        if let Logic::Native(d) = logic {
            compile(d)?;
        }
    }
    Ok(())
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
