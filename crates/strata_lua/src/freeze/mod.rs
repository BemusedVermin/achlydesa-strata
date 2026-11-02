//! The freeze step: records become typed definitions (design §4.3 step 5, §9.1).
//!
//! # Path grammar
//!
//! A *route* is the `/`-separated path from a record's root to a value. Each segment is a field
//! name, a map key or a one-based list index, exactly as the author wrote them in Lua:
//! `abilities/2/after/figure_destroyed` is the function at `abilities[2].after.figure_destroyed`.
//!
//! A [`ScriptRef`]'s `path` is `kind:name/route`, where `name` is the record's ID without its
//! pack, for example `unit:ironclad_line/abilities/2/after/figure_destroyed`. The pack is held
//! in `ScriptRef::pack`. Fields are visited in sorted order, so paths and their order do not
//! depend on Lua's string hashing.
//!
//! [`LoadError::field_path`] uses the same routes.
//!
//! # Placeholders
//!
//! Real `DefId`s are interned once every record is known. Until then, each typed struct holds
//! placeholders: its own `id` is `DefId(0)` (when the kind has one), and each cross-reference
//! is `DefId(i)` for entry `i` of [`FrozenRecord::references`]. A `ScriptRef`'s `def` is
//! `DefId(0)`, meaning the record itself. Resolving and kind-checking these is the next step.

mod schema;
mod walk;

use mlua::{Lua, Table, Value};
use serde_path_to_error::{Error, Segment};
use strata_model::{
    AbilityDef, ArmyList, DieDef, FactionDef, FeatureDef, FootprintDef, FormatDef, InventoryDef,
    MapDef, MarkerDef, RelicDef, RuleModuleDef, RulesetDef, ScenarioDef, ScriptRef, StatDef,
    TerrainDef, TestDef, TileDef, TraitsDef, UnitDef,
};

use crate::data::{DataStage, Record};
use crate::error::LoadError;
use walk::{Fault, Walk};

pub use walk::Reference;

macro_rules! definitions {
    ($($kind:literal => $variant:ident($def:ty)),* $(,)?) => {
        /// A record converted to its typed definition.
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum FrozenDef {
            $(#[doc = concat!("A `", $kind, "` record.")] $variant($def)),*
        }

        fn decode(kind: &str, value: Value) -> Option<Result<FrozenDef, Error<mlua::Error>>> {
            let source = mlua::serde::Deserializer::new(value);
            Some(match kind {
                $($kind => serde_path_to_error::deserialize(source).map(FrozenDef::$variant),)*
                _ => return None,
            })
        }
    };
}

definitions! {
    "stat" => Stat(StatDef),
    "die" => Die(DieDef),
    "marker" => Marker(MarkerDef),
    "traits" => Traits(TraitsDef),
    "faction" => Faction(FactionDef),
    "terrain" => Terrain(TerrainDef),
    "tile" => Tile(TileDef),
    "feature" => Feature(FeatureDef),
    "footprint" => Footprint(FootprintDef),
    "unit" => Unit(UnitDef),
    "ability" => Ability(AbilityDef),
    "relic" => Relic(RelicDef),
    "rule_module" => RuleModule(RuleModuleDef),
    "ruleset" => Ruleset(RulesetDef),
    "map" => Map(MapDef),
    "inventory" => Inventory(InventoryDef),
    "scenario" => Scenario(ScenarioDef),
    "format" => Format(FormatDef),
    "army" => Army(ArmyList),
    "test" => Test(TestDef),
}

/// A record after the freeze, with what the resolve step still has to patch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenRecord {
    /// The record's kind, ID and source.
    pub record: Record,
    /// The typed definition, holding placeholder `DefId`s.
    pub def: FrozenDef,
    /// Every cross-reference, indexed by placeholder.
    pub references: Vec<Reference>,
    /// Every function that became a [`ScriptRef`], in route order.
    pub scripts: Vec<ScriptRef>,
}

/// Converts every record to its typed definition, collecting every error instead of stopping at
/// the first. Records come back in `(kind, ID)` order.
pub fn freeze(stage: &DataStage) -> Result<Vec<FrozenRecord>, Vec<LoadError>> {
    let lua = stage.host().lua();
    let (mut frozen, mut errors) = (Vec::new(), Vec::new());
    for record in stage.records() {
        match stage.table(&record.kind, &record.id) {
            Some(table) => match freeze_record(lua, &table, record) {
                Ok(done) => frozen.push(done),
                Err(error) => errors.push(error),
            },
            None => errors.push(error_at(&record, "", "the record is missing".into())),
        }
    }
    if errors.is_empty() {
        Ok(frozen)
    } else {
        Err(errors)
    }
}

fn error_at(record: &Record, field_path: &str, message: String) -> LoadError {
    LoadError {
        file: record.source.file.clone(),
        line: record.source.line,
        field_path: field_path.to_owned(),
        message,
    }
}

fn freeze_record(lua: &Lua, table: &Table, record: Record) -> Result<FrozenRecord, LoadError> {
    let kind = record.kind.as_str();
    let name = record.id.split_once(':').map_or(&*record.id, |(_, n)| n);
    let mut walk = Walk::new(lua, kind, &record.source.pack, name);
    let at = |fault: Fault| error_at(&record, &fault.field_path, fault.message);
    let id = schema::has_id(kind).then(|| walk.reference("id".into(), record.id.clone(), false));
    let reserved: &[&str] = if schema::has_source(kind) {
        &["id", "source"]
    } else {
        &["id"]
    };
    let plain = walk.record(table, reserved).map_err(at)?;
    let fail = |error: mlua::Error| at(error.into());
    if let Some(id) = id {
        plain.raw_set("id", id).map_err(fail)?;
    }
    if schema::has_source(kind) {
        let source = lua.create_table().map_err(fail)?;
        source.raw_set("file", &*record.source.file).map_err(fail)?;
        source.raw_set("line", record.source.line).map_err(fail)?;
        plain.raw_set("source", source).map_err(fail)?;
    }
    let (references, scripts) = (walk.references, walk.scripts);
    let decoded = decode(kind, Value::Table(plain));
    let decoded = decoded.unwrap_or_else(|| unreachable!("every kind has a schema"));
    match decoded {
        Ok(def) => Ok(FrozenRecord {
            record,
            def,
            references,
            scripts,
        }),
        Err(error) => {
            let (field_path, message) = describe(&error, &references);
            Err(error_at(&record, &field_path, message))
        }
    }
}

/// The route and message of a serde error. A missing or unknown field is named in the route.
fn describe(error: &Error<mlua::Error>, references: &[Reference]) -> (String, String) {
    let message = match error.inner() {
        mlua::Error::DeserializeError(text) => text.clone(),
        other => other.to_string(),
    };
    let mut route: Vec<String> = Vec::new();
    for segment in error.path().iter() {
        let part = match segment {
            Segment::Seq { index } => (index + 1).to_string(),
            Segment::Map { key } => placeholder_name(key, references),
            Segment::Enum { variant } => variant.clone(),
            Segment::Unknown => unnamed_key(&route.join("/"), references),
        };
        route.push(part);
    }
    let missing = message.strip_prefix("missing field `");
    if let Some(field) = missing.and_then(|rest| rest.split('`').next()) {
        route.push(field.to_owned());
    }
    (route.join("/"), message)
}

/// A map key that is a placeholder reads back as the name the author wrote.
fn placeholder_name(key: &str, references: &[Reference]) -> String {
    let reference = key.parse::<usize>().ok().and_then(|i| references.get(i));
    match reference {
        Some(r) if r.key => r.target.clone(),
        _ => key.to_owned(),
    }
}

/// serde does not report integer map keys, so a reference-keyed map reports its entry by name
/// only when there is a single one; otherwise the entry reads `*`.
fn unnamed_key(parent: &str, references: &[Reference]) -> String {
    let in_map =
        |r: &&Reference| r.key && r.field_path.rsplit_once('/').map(|(p, _)| p) == Some(parent);
    let mut entries = references.iter().filter(in_map);
    match (entries.next(), entries.next()) {
        (Some(only), None) => only.target.clone(),
        _ => "*".to_owned(),
    }
}
