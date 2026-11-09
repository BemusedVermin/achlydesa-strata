//! Card text from a template and an ability's parameters (design §8.4).

use std::collections::BTreeMap;

use strata_kernel::{Sym, Value};
use strata_model::Descriptor;

use super::{DescriptorError, compile};

/// A template could not be filled.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CardTextError {
    /// A brace has no partner: a `{` is never closed, or a `}` was never opened. Write `{{` or
    /// `}}` for a literal brace.
    #[error("unmatched brace at byte {0}")]
    Unmatched(usize),
    /// A placeholder names no parameter.
    #[error("the template names `{{{0}}}`, but there is no such parameter")]
    Unknown(String),
    /// A placeholder asks for a kind of phrase that does not exist.
    #[error("`{{{param}:{kind}}}`: the kind is `sel`, `cond`, `val` or `eff`")]
    Kind {
        /// The parameter.
        param: String,
        /// The kind asked for.
        kind: String,
    },
    /// A parameter has no wording, such as a Lua function.
    #[error("`{param}` cannot be worded: {reason}")]
    Unworded {
        /// The parameter.
        param: String,
        /// Why not.
        reason: String,
    },
}

/// Fills `template` with the phrases of `params`, so `"{to} add {amount} to their {stat}."`
/// becomes `"Adjacent friendly soldier figures add 1 to their defense."`.
///
/// `{name}` is replaced by the parameter's phrase: a descriptor's phrase for a tagged map, the
/// number for an integer, the text for a string, and a list joined as `a, b and c`. A bare
/// literal that stands for a descriptor is worded as one with a kind: `{to:sel}` reads the string
/// `"self"` as a selector (`this figure`), and `:cond`, `:val` and `:eff` work alike. `{{` and `}}`
/// are literal braces. A phrase that starts the text or a sentence is capitalized. The function
/// is pure: the same template and parameters always give the same text.
pub fn card_text(template: &str, params: &BTreeMap<Sym, Value>) -> Result<String, CardTextError> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(at) = rest.find(['{', '}']) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        let offset = template.len() - tail.len();
        let close = match tail.strip_prefix('{').and_then(|t| t.find('}')) {
            Some(close) => close,
            None => return Err(CardTextError::Unmatched(offset)),
        };
        let inner = &tail[1..=close];
        // A `{` inside a placeholder is never closed: `{a{b}` leaves the first one open.
        if inner.contains('{') {
            return Err(CardTextError::Unmatched(offset));
        }
        let (name, kind) = inner
            .split_once(':')
            .map_or((inner, None), |(n, k)| (n, Some(k)));
        let value = params
            .get(name)
            .ok_or_else(|| CardTextError::Unknown(name.to_owned()))?;
        let phrase = word(name, value, kind)?;
        if starts_sentence(&out) {
            out.push_str(&capitalize(&phrase));
        } else {
            out.push_str(&phrase);
        }
        rest = &tail[close + 2..];
    }
    out.push_str(rest);
    Ok(capitalize(&out))
}

/// Whether text appended to `before` starts a sentence.
fn starts_sentence(before: &str) -> bool {
    let trimmed = before.trim_end();
    trimmed.is_empty() || trimmed.ends_with(['.', '!', '?'])
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The phrase of one parameter.
fn word(param: &str, value: &Value, kind: Option<&str>) -> Result<String, CardTextError> {
    let unworded = |reason: String| CardTextError::Unworded {
        param: param.to_owned(),
        reason,
    };
    let phrase = |d: Result<super::Node, DescriptorError>| {
        d.map(|node| node.phrase())
            .map_err(|e| unworded(e.to_string()))
    };
    if let Some(kind) = kind {
        if !["sel", "cond", "val", "eff"].contains(&kind) {
            let (param, kind) = (param.to_owned(), kind.to_owned());
            return Err(CardTextError::Kind { param, kind });
        }
        // A bare string names a selector or an effect; a condition or value has no bare name.
        let tagged = match value {
            Value::Str(name) if kind == "sel" || kind == "eff" => {
                let tag = (Sym::new(kind), Value::Str(name.clone()));
                Value::Map(BTreeMap::from([tag]))
            }
            Value::Str(_) => {
                return Err(unworded(format!("a string is not a `{kind}` descriptor")));
            }
            _ => value.clone(),
        };
        let node = compile(&Descriptor(tagged)).map_err(|e| unworded(e.to_string()))?;
        let found = node_kind(&node);
        if found != kind {
            let what = match value {
                Value::Map(_) => format!("a `{found}` descriptor"),
                other => format!("a {}", other.type_name()),
            };
            return Err(unworded(format!("{what} is not a `{kind}` descriptor")));
        }
        return Ok(node.phrase());
    }
    match value {
        Value::Int(n) => Ok(n.to_string()),
        Value::Str(s) => Ok(s.as_str().to_owned()),
        Value::List(items) => {
            let words = items.iter().map(|v| word(param, v, None));
            let words = words.collect::<Result<Vec<_>, _>>()?;
            Ok(match words.split_last() {
                None => String::new(),
                Some((last, [])) => last.clone(),
                Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
            })
        }
        Value::Map(_) | Value::Bool(_) => phrase(compile(&Descriptor(value.clone()))),
        other => Err(CardTextError::Unworded {
            param: param.to_owned(),
            reason: format!("a {} has no wording", other.type_name()),
        }),
    }
}

/// The namespace of a compiled descriptor: `sel`, `cond`, `val` or `eff`.
fn node_kind(node: &super::Node) -> &'static str {
    match node {
        super::Node::Sel(_) => "sel",
        super::Node::Cond(_) => "cond",
        super::Node::Val(_) => "val",
        super::Node::Eff(_) => "eff",
    }
}

#[cfg(test)]
mod tests;
