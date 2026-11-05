//! Copies a record into a plain table: functions become `ScriptRef`s and references become
//! placeholder `DefId`s.

use mlua::{Lua, Table, Value};
use strata_kernel::{DefId, PackId, Sym};
use strata_model::ScriptRef;

use super::schema::{self, Site};
use crate::convert::{EntryError, Key, sorted_entries};

const MAX_DEPTH: usize = 64;

/// A cross-reference found in a record. Its placeholder `DefId` in the typed struct is the
/// index of this entry in [`FrozenRecord::references`](super::FrozenRecord::references).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    /// Route to the field, in the grammar described in [`freeze`](super::freeze).
    pub field_path: String,
    /// The string as authored: a full or bare ID.
    pub target: String,
    /// Whether the reference is a map key rather than a value.
    pub key: bool,
    /// The kind of definition the reference must name.
    pub kind: String,
}

/// A value that cannot be frozen, and where it sits.
pub(super) struct Fault {
    pub field_path: String,
    pub message: String,
}

impl From<mlua::Error> for Fault {
    fn from(error: mlua::Error) -> Self {
        Self {
            field_path: String::new(),
            message: error.to_string(),
        }
    }
}

pub(super) struct Walk<'a> {
    lua: &'a Lua,
    kind: &'a str,
    pack: &'a PackId,
    name: &'a str,
    route: Vec<String>,
    own: DefId,
    resolved: Option<&'a [DefId]>,
    pub references: Vec<Reference>,
    pub scripts: Vec<ScriptRef>,
}

impl<'a> Walk<'a> {
    pub fn new(lua: &'a Lua, kind: &'a str, pack: &'a PackId, name: &'a str) -> Self {
        Self {
            lua,
            kind,
            pack,
            name,
            route: Vec::new(),
            own: DefId(0),
            resolved: None,
            references: Vec::new(),
            scripts: Vec::new(),
        }
    }

    /// Makes the walk write real `DefId`s instead of placeholders: `own` for the record itself
    /// and `resolved[i]` for the `i`th reference.
    pub fn resolved(mut self, own: DefId, resolved: &'a [DefId]) -> Self {
        self.own = own;
        self.resolved = Some(resolved);
        self
    }

    /// Records a reference and returns its placeholder, or its `DefId` once resolved.
    pub fn reference(
        &mut self,
        field_path: String,
        target: String,
        key: bool,
        kind: &str,
    ) -> Value {
        let index = self.references.len();
        let reference = Reference {
            field_path,
            target,
            key,
            kind: kind.to_owned(),
        };
        self.references.push(reference);
        let value = self.resolved.map_or(index as u32, |ids| ids[index].0);
        Value::Integer(i64::from(value))
    }

    /// Copies `record`, leaving out the top-level fields in `reserved`.
    pub fn record(&mut self, record: &Table, reserved: &[&str]) -> Result<Table, Fault> {
        self.table(record, reserved, 0)
    }

    fn table(&mut self, source: &Table, reserved: &[&str], depth: usize) -> Result<Table, Fault> {
        if depth >= MAX_DEPTH {
            return Err(self.fault("table nesting is too deep or cyclic".into()));
        }
        let copy = self.lua.create_table()?;
        for (key, value) in self.entries(source)? {
            if matches!(&key, Key::Str(s) if depth == 0 && reserved.contains(&s.as_str())) {
                continue;
            }
            let segment = match &key {
                Key::Int(i) => i.to_string(),
                Key::Str(s) => s.clone(),
            };
            self.route.push(segment);
            let (key, value) = (self.key(key)?, self.value(value, depth)?);
            self.route.pop();
            copy.raw_set(key, value)?;
        }
        Ok(copy)
    }

    /// The entries of `table` in key order, or a fault naming a key that cannot be one.
    fn entries(&self, table: &Table) -> Result<Vec<(Key, Value)>, Fault> {
        sorted_entries(self.lua, table).map_err(|error| match error {
            EntryError::Lua(error) => error.into(),
            EntryError::BadKey(kind) => self.fault(format!("a {kind} cannot be a key")),
            EntryError::NotUtf8 => self.fault("a key is not valid UTF-8".into()),
        })
    }

    /// `key` as a Lua value, as authored.
    fn plain_key(&self, key: Key) -> Result<Value, Fault> {
        Ok(match key {
            Key::Int(i) => Value::Integer(i),
            Key::Str(s) => Value::String(self.lua.create_string(&s)?),
        })
    }

    fn key(&mut self, key: Key) -> Result<Value, Fault> {
        match (key, schema::reference(self.kind, &self.route)) {
            (Key::Str(s), Some((Site::Key, target))) => {
                Ok(self.reference(self.route.join("/"), s, true, target))
            }
            (key, _) => self.plain_key(key),
        }
    }

    fn value(&mut self, value: Value, depth: usize) -> Result<Value, Fault> {
        if schema::is_value(self.kind, &self.route) {
            return self.tagged(value, depth);
        }
        if schema::is_logic(self.kind, &self.route) && !matches!(value, Value::Function(_)) {
            return self.native(value, depth);
        }
        Ok(match value {
            Value::Function(_) => self.script()?,
            Value::Table(table) => Value::Table(self.table(&table, &[], depth + 1)?),
            Value::String(s) => match schema::reference(self.kind, &self.route) {
                Some((Site::Value, target)) => {
                    let authored = s.to_str()?.to_owned();
                    self.reference(self.route.join("/"), authored, false, target)
                }
                _ => Value::String(s),
            },
            Value::UserData(_) | Value::Thread(_) | Value::LightUserData(_) => {
                return Err(self.fault(format!("a {} cannot be frozen", value.type_name())));
            }
            other => other,
        })
    }

    /// Rewrites plain Lua data as the externally tagged form `Value` deserializes from.
    fn tagged(&mut self, value: Value, depth: usize) -> Result<Value, Fault> {
        let wrap = |tag: &str, inner: Value| -> Result<Value, Fault> {
            let table = self.lua.create_table()?;
            table.raw_set(tag, inner)?;
            Ok(Value::Table(table))
        };
        match value {
            Value::Nil => Ok(Value::String(self.lua.create_string("Nil")?)),
            Value::Boolean(_) => wrap("Bool", value),
            Value::Integer(_) => wrap("Int", value),
            Value::String(_) => wrap("Str", value),
            Value::Table(table) if depth < MAX_DEPTH => {
                let items = self.lua.create_table()?;
                let (mut list, mut map) = (true, false);
                for (key, item) in self.entries(&table)? {
                    let item = self.tagged(item, depth + 1)?;
                    map |= matches!(key, Key::Str(_));
                    list &= matches!(key, Key::Int(_));
                    items.raw_set(self.plain_key(key)?, item)?;
                }
                wrap(
                    if map && !list { "Map" } else { "List" },
                    Value::Table(items),
                )
            }
            other => Err(self.fault(format!("a {} is not plain data", other.type_name()))),
        }
    }

    /// Wraps plain data in a `Logic` field as `Logic::Native`, the descriptor the Rust evaluator
    /// reads: `{ sel = 'allies' }`, `true` or `1` as written, or `{ native = 'name' }` for logic
    /// a module registered in Rust.
    fn native(&mut self, value: Value, depth: usize) -> Result<Value, Fault> {
        let logic = self.lua.create_table()?;
        logic.raw_set("native", self.tagged(value, depth)?)?;
        Ok(Value::Table(logic))
    }

    /// Replaces the function at the current route with a `ScriptRef`, wrapped as
    /// `Logic::Script` unless the field is typed as a bare `ScriptRef`.
    fn script(&mut self) -> Result<Value, Fault> {
        let route = self.route.join("/");
        let path = format!("{}:{}/{route}", self.kind, self.name);
        let script = ScriptRef {
            pack: self.pack.clone(),
            def: self.own,
            path: Sym::new(&path),
        };
        let table = self.lua.create_table()?;
        table.raw_set("pack", script.pack.as_str())?;
        table.raw_set("def", script.def.0)?;
        table.raw_set("path", path)?;
        self.scripts.push(script);
        if schema::bare_script(self.kind, &self.route) {
            return Ok(Value::Table(table));
        }
        let logic = self.lua.create_table()?;
        logic.raw_set("script", table)?;
        Ok(Value::Table(logic))
    }

    fn fault(&self, message: String) -> Fault {
        Fault {
            field_path: self.route.join("/"),
            message,
        }
    }
}
