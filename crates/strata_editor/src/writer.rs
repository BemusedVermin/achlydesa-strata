//! The Lua map writer: a `return map { ... }` literal with stable key order (design §11.1, §14.1).

use std::fmt::Write as _;

use strata_kernel::{Hex, Value};
use strata_model::MapDef;
use thiserror::Error;

/// Why a map could not be written.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum WriteError {
    /// A free-form `meta` value has no Lua literal form.
    #[error("meta entry `{0}` holds a definition handle, which has no Lua form")]
    UnsupportedMeta(String),
}

const INDENT: &str = "  ";
const KEYWORDS: [&str; 22] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// Writes `map` as Lua source under the authored name `id`.
///
/// Fields come in a fixed order, maps are written in key order and each piece, feature and zone
/// takes one line, so the same map always produces the same bytes. Empty optional fields are
/// left out.
pub fn write_map(map: &MapDef, id: &str) -> Result<String, WriteError> {
    let mut out = String::from("return map {\n");
    let authors: Vec<String> = map.authors.iter().map(|a| quote(a)).collect();
    let _ = writeln!(
        out,
        "{INDENT}id = {}, name = {}, authors = {}, orientation = {},",
        quote(id),
        quote(&map.name.0),
        braces(&authors),
        quote(map.orientation.as_str()),
    );
    let rows: Vec<[String; 3]> = map
        .pieces
        .iter()
        .map(|p| {
            [
                named("piece", p.piece.as_str()),
                cell(p.at),
                p.rot.steps().to_string(),
            ]
        })
        .collect();
    let levels: Vec<String> = map.pieces.iter().map(|p| p.level.to_string()).collect();
    write_list(&mut out, "pieces", &rows, Some(&levels));
    let rows: Vec<[String; 3]> = map
        .features
        .iter()
        .map(|f| {
            [
                named("feature", f.feature.as_str()),
                cell(f.at),
                f.rot.steps().to_string(),
            ]
        })
        .collect();
    write_list(&mut out, "features", &rows, None);
    write_zones(&mut out, map);
    if !map.relic_spots.is_empty() {
        let spots: Vec<String> = map
            .relic_spots
            .iter()
            .map(|(name, at)| format!("{} = {}", key(name.as_str()), cell(*at)))
            .collect();
        let _ = writeln!(out, "{INDENT}relic_spots = {{ {} }},", spots.join(", "));
    }
    write_constraints(&mut out, map);
    if !map.tags.is_empty() {
        let tags: Vec<String> = map.tags.iter().map(|t| quote(t.as_str())).collect();
        let _ = writeln!(out, "{INDENT}tags = {{ {} }},", tags.join(", "));
    }
    if !map.meta.is_empty() {
        let mut meta = Vec::new();
        for (name, value) in &map.meta {
            let text = value_literal(value)
                .ok_or_else(|| WriteError::UnsupportedMeta(name.as_str().to_owned()))?;
            meta.push(format!("{} = {text}", key(name.as_str())));
        }
        let _ = writeln!(out, "{INDENT}meta = {{ {} }},", meta.join(", "));
    }
    out.push_str("}\n");
    Ok(out)
}

/// Writes `name = { ... }` with one aligned line per row; rows are `{ piece = .., at = .., rot = .. }`.
fn write_list(out: &mut String, name: &str, rows: &[[String; 3]], levels: Option<&[String]>) {
    if rows.is_empty() {
        if levels.is_some() {
            let _ = writeln!(out, "{INDENT}{name} = {{}},");
        }
        return;
    }
    let _ = writeln!(out, "{INDENT}{name} = {{");
    // Each cell carries its trailing comma so padding lands after it.
    let width = |i: usize| rows.iter().map(|r| r[i].len() + 1).max().unwrap_or(0);
    let (name_width, at_width) = (width(0), width(1));
    for (i, row) in rows.iter().enumerate() {
        let level = levels.map_or(String::new(), |l| format!(", level = {}", l[i]));
        let _ = writeln!(
            out,
            "{INDENT}{INDENT}{{ {:<name_width$} at = {:<at_width$} rot = {}{level} }},",
            format!("{},", row[0]),
            format!("{},", row[1]),
            row[2],
        );
    }
    let _ = writeln!(out, "{INDENT}}},");
}

fn write_zones(out: &mut String, map: &MapDef) {
    if map.zones.is_empty() {
        return;
    }
    let _ = writeln!(out, "{INDENT}zones = {{");
    let keys: Vec<String> = map.zones.keys().map(|k| key(k.as_str())).collect();
    let width = keys.iter().map(String::len).max().unwrap_or(0);
    for (name, cells) in keys.iter().zip(map.zones.values()) {
        let cells: Vec<String> = cells.iter().map(|c| cell(*c)).collect();
        let _ = writeln!(
            out,
            "{INDENT}{INDENT}{name:<width$} = {{ {} }},",
            cells.join(", ")
        );
    }
    let _ = writeln!(out, "{INDENT}}},");
}

fn write_constraints(out: &mut String, map: &MapDef) {
    let mut fields = Vec::new();
    if let Some(inventory) = &map.constraints.inventory {
        fields.push(format!("inventory = {}", quote(inventory.as_str())));
    }
    if map.constraints.require_support {
        fields.push("require_support = true".to_owned());
    }
    if !fields.is_empty() {
        let _ = writeln!(out, "{INDENT}constraints = {{ {} }},", fields.join(", "));
    }
}

/// `{ a, b }`, or `{}` when there is nothing to list.
fn braces(items: &[String]) -> String {
    if items.is_empty() {
        "{}".to_owned()
    } else {
        format!("{{ {} }}", items.join(", "))
    }
}

/// `field = "text"`.
fn named(field: &str, text: &str) -> String {
    format!("{field} = {}", quote(text))
}

fn cell(at: Hex) -> String {
    format!("{{ q = {}, r = {} }}", at.q, at.r)
}

/// A table key: a bare name when Lua allows it, otherwise `["name"]`.
fn key(name: &str) -> String {
    let mut chars = name.chars();
    let starts = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let rest = chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if starts && rest && !KEYWORDS.contains(&name) {
        name.to_owned()
    } else {
        format!("[{}]", quote(name))
    }
}

/// A double-quoted Lua string. Control characters use decimal escapes, which Lua reads back
/// byte for byte.
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => {
                let mut buf = [0; 4];
                for byte in c.encode_utf8(&mut buf).bytes() {
                    let _ = write!(out, "\\{byte:03}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The literal for `value` in the externally tagged form the loader reads (`{ Int = 3 }`), or
/// `None` if it holds a definition handle.
fn value_literal(value: &Value) -> Option<String> {
    Some(match value {
        Value::Nil => quote("Nil"),
        Value::Bool(b) => format!("{{ Bool = {b} }}"),
        Value::Int(i) => format!("{{ Int = {i} }}"),
        Value::Str(s) => format!("{{ Str = {} }}", quote(s.as_str())),
        Value::Id(_) => return None,
        Value::List(items) => {
            let items: Option<Vec<String>> = items.iter().map(value_literal).collect();
            format!("{{ List = {{ {} }} }}", items?.join(", "))
        }
        Value::Map(entries) => {
            let mut parts = Vec::new();
            for (name, item) in entries {
                parts.push(format!("{} = {}", key(name.as_str()), value_literal(item)?));
            }
            format!("{{ Map = {{ {} }} }}", parts.join(", "))
        }
    })
}
