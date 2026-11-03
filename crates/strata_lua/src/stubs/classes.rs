//! The authoring surface of every constructor, mirroring the `strata_model` definitions.
//!
//! Fields the loader fills in (`source`, and the `id` of kinds without one) are left out.
//! Deeply nested descriptor trees are typed `any` until their vocabulary is typed in Rust.

/// The type of a field as an author writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    /// An integer.
    Int,
    /// A boolean.
    Bool,
    /// A string.
    Str,
    /// A string naming a definition of the given kind.
    Ref(&'static str),
    /// One of a fixed set of strings.
    Enum(&'static [&'static str]),
    /// A list.
    List(&'static Ty),
    /// A table with the given key and value types.
    Map(&'static Ty, &'static Ty),
    /// A nested class.
    Class(&'static str),
    /// A Lua function.
    Func,
    /// Any value.
    Any,
}

/// One field of a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    /// The key an author writes.
    pub name: &'static str,
    /// The field's type.
    pub ty: Ty,
    /// Whether the loader rejects a record without it.
    pub required: bool,
}

/// A named group of fields.
#[derive(Clone, Copy, Debug)]
pub struct Class {
    /// The class name, without the `strata.` prefix.
    pub name: &'static str,
    /// Its fields.
    pub fields: &'static [Field],
}

/// A constructor: its function name, class and the nested classes it uses.
pub(super) struct Kind {
    pub(super) name: &'static str,
    pub(super) class: &'static str,
    pub(super) fields: &'static [Field],
    pub(super) nested: &'static [Class],
    /// Whether `tags` and `meta` are accepted.
    pub(super) common: bool,
}

const fn req(name: &'static str, ty: Ty) -> Field {
    Field {
        name,
        ty,
        required: true,
    }
}

const fn opt(name: &'static str, ty: Ty) -> Field {
    Field {
        name,
        ty,
        required: false,
    }
}

const TAGS: Ty = Ty::List(&Ty::Str);
const COUNTS: Ty = Ty::Map(&Ty::Str, &Ty::Int);
const ANY_MAP: Ty = Ty::Map(&Ty::Str, &Ty::Any);
const HEX: Ty = Ty::List(&Ty::Int);
const HEXES: Ty = Ty::List(&HEX);
const LOGIC: Ty = Ty::List(&Ty::Any);
const MODULE_REFS: Ty = Ty::List(&Ty::Class("ModuleRef"));

/// Fields every kind except `ability`, `army` and `test` accepts.
pub(super) const COMMON: [Field; 2] = [opt("tags", TAGS), opt("meta", ANY_MAP)];

const ID: Field = req("id", Ty::Str);
const NAME: Field = req("name", Ty::Str);

const MODULE_REF: Class = Class {
    name: "ModuleRef",
    fields: &[req("id", Ty::Str), opt("config", ANY_MAP)],
};

const fn kind(
    name: &'static str,
    class: &'static str,
    fields: &'static [Field],
    nested: &'static [Class],
) -> Kind {
    Kind {
        name,
        class,
        fields,
        nested,
        common: true,
    }
}

const fn bare(name: &'static str, class: &'static str, fields: &'static [Field]) -> Kind {
    Kind {
        name,
        class,
        fields,
        nested: &[],
        common: false,
    }
}

/// Every constructor, in the order the data stage registers them.
pub(super) const KINDS: [Kind; 20] = [
    kind(
        "stat",
        "Stat",
        &[
            ID,
            NAME,
            req("min", Ty::Int),
            req("max", Ty::Int),
            opt("abbrev", Ty::Str),
            opt("order", Ty::Int),
        ],
        &[],
    ),
    kind(
        "die",
        "Die",
        &[ID, NAME, opt("faces", TAGS), opt("range", HEX)],
        &[],
    ),
    kind(
        "marker",
        "Marker",
        &[
            ID,
            NAME,
            req("stacking", Ty::Str),
            req("visibility", Ty::Str),
        ],
        &[],
    ),
    kind("traits", "Traits", &[ID, NAME, req("keys", ANY_MAP)], &[]),
    kind(
        "faction",
        "Faction",
        &[ID, NAME, req("color", Ty::Str), opt("emblem", Ty::Str)],
        &[],
    ),
    kind(
        "terrain",
        "Terrain",
        &[
            ID,
            NAME,
            req("standable", Ty::Bool),
            opt("abilities", Ty::List(&Ty::Ref("ability"))),
        ],
        &[],
    ),
    kind(
        "tile",
        "Tile",
        &[
            ID,
            NAME,
            req("terrain", Ty::Ref("terrain")),
            req("thickness", Ty::Int),
            req("shape", HEXES),
        ],
        &[],
    ),
    kind(
        "feature",
        "Feature",
        &[
            ID,
            NAME,
            req("blocks_los", Ty::Bool),
            req("cells", Ty::List(&Ty::Class("FeatureCell"))),
        ],
        &[
            Class {
                name: "FeatureCell",
                fields: &[
                    req("at", HEX),
                    req("segments", Ty::List(&Ty::Class("Segment"))),
                ],
            },
            Class {
                name: "Segment",
                fields: &[
                    req("from", Ty::Int),
                    req("to", Ty::Int),
                    opt("top", Ty::Bool),
                ],
            },
        ],
    ),
    kind(
        "footprint",
        "Footprint",
        &[
            ID,
            opt("name", Ty::Str),
            req("cells", HEXES),
            req("move", Ty::Enum(&["rigid", "snake"])),
            opt("max_spread", Ty::Int),
        ],
        &[],
    ),
    kind(
        "unit",
        "Unit",
        &[
            ID,
            NAME,
            req("kind", Ty::Str),
            req("rarity", Ty::Str),
            opt("faction", Ty::Ref("faction")),
            opt("traits", Ty::Map(&Ty::Str, &Ty::Str)),
            req("stats", Ty::Map(&Ty::Ref("stat"), &Ty::Int)),
            req("figures", Ty::List(&Ty::Class("Figure"))),
            opt("abilities", LOGIC),
            opt("card", Ty::Class("CardInfo")),
        ],
        &[
            Class {
                name: "LosProfile",
                fields: &[
                    req("target", Ty::Int),
                    req("hit_zone", HEX),
                    req("samples", TAGS),
                ],
            },
            Class {
                name: "Figure",
                fields: &[
                    req("id", Ty::Str),
                    req("count", Ty::Int),
                    req("footprint", Ty::Ref("footprint")),
                    req("height", Ty::Int),
                    req("los", Ty::Class("LosProfile")),
                    opt("visual", Ty::Str),
                ],
            },
            Class {
                name: "CardInfo",
                fields: &[opt("art", Ty::Str), opt("flavor", Ty::Str)],
            },
        ],
    ),
    bare(
        "ability",
        "Ability",
        &[
            NAME,
            opt("text", Ty::Str),
            req(
                "scope",
                Ty::Enum(&["figure", "unit", "army", "holder", "occupant"]),
            ),
            opt("archetype", ANY_MAP),
            opt("modifiers", LOGIC),
            opt("actions", LOGIC),
            opt("replacements", LOGIC),
            opt("triggers", LOGIC),
            opt("reactions", LOGIC),
            opt("vars", ANY_MAP),
        ],
    ),
    kind(
        "relic",
        "Relic",
        &[
            ID,
            NAME,
            req("mode", Ty::Enum(&["while_occupied", "on_claim"])),
            req("claim", Ty::Str),
            opt("face_down_allowed", Ty::Bool),
            opt("grants", ANY_MAP),
            opt("effect", Ty::Any),
            opt("visual", Ty::Str),
        ],
        &[],
    ),
    kind(
        "rule_module",
        "RuleModule",
        &[
            ID,
            opt("name", Ty::Str),
            opt("slot", Ty::Str),
            opt("depends", TAGS),
            opt("config", Ty::Map(&Ty::Str, &Ty::Class("ConfigField"))),
            req("register", Ty::Func),
        ],
        &[Class {
            name: "ConfigField",
            fields: &[req("type", Ty::Str), opt("default", Ty::Any)],
        }],
    ),
    kind(
        "ruleset",
        "Ruleset",
        &[
            ID,
            opt("name", Ty::Str),
            opt("extends", Ty::Str),
            opt("stats", TAGS),
            opt("dice", TAGS),
            opt("slots", Ty::Map(&Ty::Str, &Ty::Class("ModuleRef"))),
            opt("addons", MODULE_REFS),
            opt("abilities", Ty::Bool),
            opt("undo", Ty::Enum(&["until_random_or_reveal"])),
        ],
        &[MODULE_REF],
    ),
    kind(
        "map",
        "Map",
        &[
            ID,
            NAME,
            opt("authors", TAGS),
            req("orientation", Ty::Str),
            req("pieces", LOGIC),
            opt("features", LOGIC),
            opt("zones", Ty::Map(&Ty::Str, &HEXES)),
            opt("relic_spots", Ty::Map(&Ty::Str, &HEX)),
            opt("constraints", ANY_MAP),
        ],
        &[],
    ),
    kind(
        "inventory",
        "Inventory",
        &[ID, NAME, req("tiles", COUNTS), opt("features", COUNTS)],
        &[],
    ),
    kind(
        "scenario",
        "Scenario",
        &[
            ID,
            NAME,
            req("map", Ty::Str),
            req("players", ANY_MAP),
            req("slots", LOGIC),
            opt("army", LOGIC),
            opt("relics", LOGIC),
            opt("victory", LOGIC),
            opt("addons", MODULE_REFS),
            opt("after", Ty::Map(&Ty::Str, &Ty::Func)),
        ],
        &[MODULE_REF],
    ),
    kind(
        "format",
        "Format",
        &[
            ID,
            NAME,
            req("ruleset", Ty::Str),
            opt("army", LOGIC),
            req("selection", Ty::Str),
            req("scenarios", Ty::Any),
            opt("round_limit", Ty::Int),
            opt("clock_minutes", Ty::Int),
            opt("tiebreakers", TAGS),
        ],
        &[],
    ),
    bare(
        "army",
        "Army",
        &[
            NAME,
            req("format", Ty::Str),
            req("packs", Ty::Map(&Ty::Str, &Ty::Str)),
            req("units", LOGIC),
        ],
    ),
    bare(
        "test",
        "Test",
        &[
            NAME,
            opt("ruleset", Ty::Str),
            opt("map", Ty::Str),
            opt("place", Ty::Any),
            opt("steps", Ty::Any),
        ],
    ),
];
