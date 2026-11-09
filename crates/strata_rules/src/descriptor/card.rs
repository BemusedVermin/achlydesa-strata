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
    let phrase = |d: Result<super::Node, DescriptorError>| {
        d.map(|node| node.phrase())
            .map_err(|e| CardTextError::Unworded {
                param: param.to_owned(),
                reason: e.to_string(),
            })
    };
    if let Some(kind) = kind {
        if !["sel", "cond", "val", "eff"].contains(&kind) {
            let (param, kind) = (param.to_owned(), kind.to_owned());
            return Err(CardTextError::Kind { param, kind });
        }
        let tagged = match value {
            Value::Map(m) if m.contains_key(kind) => value.clone(),
            Value::Str(name) if kind != "cond" && kind != "val" => {
                let tag = (Sym::new(kind), Value::Str(name.clone()));
                Value::Map(BTreeMap::from([tag]))
            }
            _ => value.clone(),
        };
        return phrase(compile(&Descriptor(tagged)));
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

#[cfg(test)]
mod tests;
