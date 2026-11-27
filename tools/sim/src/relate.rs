//! `weft-sim relate`: a metamorphic relation between two experiments (technique 5 of the skill
//! `ess:hardening`, docs/hardening/pass-3.md). Runs the setup of each on a model of its own and
//! compares the rows of every parameterless read-your-writes view after it.
//!
//! Values are compared after naming: a string the setup bound to a variable reads `$<variable>`;
//! any other minted identifier (`00000000-0000-4000-8000-<tick>`, from the interpreter's tick
//! counter, which moves with every request sent) reads `$minted`, and a generated `Timestamp` of
//! that counter (a day of January 2020) reads `$minted-time`, a generated text (`generated-<n>`)
//! reads `$minted-text`. Rows are compared as a multiset
//! per view. With `observe`, only the rows that name an observed variable are compared (what one
//! party sees of its own); without it, `hide` leaves out every row that names a hidden variable.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::simulate::Fingerprint;

/// Which rows a relation compares.
#[derive(Default)]
pub struct Scope {
    pub hide: BTreeSet<String>,
    pub observe: BTreeSet<String>,
    /// Row fields left out of the comparison (a known, recorded difference).
    pub ignore: BTreeSet<String>,
}

/// The rows of one run, named and scoped: per view, each row as canonical JSON.
pub struct Named {
    pub views: BTreeMap<String, Vec<String>>,
    pub rows: usize,
    pub dropped: usize,
}

const MINTED: &str = "00000000-0000-4000-8000-";

/// A generated `Timestamp` of the interpreter: a day of January 2020 at midnight, from the same
/// tick counter (`ess_conformance::interpret::values::mint`).
fn minted_time(s: &str) -> bool {
    s.len() == 20
        && s.starts_with("2020-01-")
        && s.ends_with("T00:00:00Z")
        && s[8..10].bytes().all(|b| b.is_ascii_digit())
}

fn rename(value: &Value, names: &BTreeMap<&str, &str>, seen: &mut BTreeSet<String>) -> Value {
    match value {
        Value::String(s) => {
            if let Some(n) = names.get(s.as_str()) {
                seen.insert((*n).to_owned());
                Value::String(format!("${n}"))
            } else if s.starts_with(MINTED) {
                Value::String("$minted".into())
            } else if minted_time(s) {
                Value::String("$minted-time".into())
            } else if s
                .strip_prefix("generated-")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            {
                Value::String("$minted-text".into())
            } else {
                value.clone()
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(|v| rename(v, names, seen)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), rename(v, names, seen)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

/// Names the values of `rows` by `vars` and keeps the rows `scope` compares.
pub fn name_rows(rows: &Fingerprint, vars: &BTreeMap<String, Value>, scope: &Scope) -> Named {
    // A value bound to several variables takes the first name in order.
    let mut names: BTreeMap<&str, &str> = BTreeMap::new();
    for (k, v) in vars {
        if let Value::String(s) = v {
            names.entry(s.as_str()).or_insert(k.as_str());
        }
    }
    let mut views = BTreeMap::new();
    let (mut total, mut dropped) = (0, 0);
    for (view, list) in rows {
        let mut kept = Vec::new();
        for row in list {
            total += 1;
            let mut seen = BTreeSet::new();
            let mut named = rename(
                &serde_json::to_value(row).unwrap_or(Value::Null),
                &names,
                &mut seen,
            );
            if let Value::Object(o) = &mut named {
                o.retain(|k, _| !scope.ignore.contains(k));
            }
            let hidden = seen.iter().any(|n| scope.hide.contains(n));
            let observed = seen.iter().any(|n| scope.observe.contains(n));
            let left_out = if scope.observe.is_empty() {
                hidden
            } else {
                !observed
            };
            if left_out {
                dropped += 1;
                continue;
            }
            kept.push(named.to_string());
        }
        kept.sort();
        views.insert(view.clone(), kept);
    }
    Named {
        views,
        rows: total,
        dropped,
    }
}

/// The rows of each view found in one run more often than in the other, as
/// `<view>: only in <label>: <row>` lines; empty when the relation holds.
pub fn differences(a: &Named, b: &Named, labels: (&str, &str)) -> Vec<String> {
    let mut out = Vec::new();
    let empty = Vec::new();
    let all: BTreeSet<&String> = a.views.keys().chain(b.views.keys()).collect();
    for view in all {
        let mut count: BTreeMap<&str, i64> = BTreeMap::new();
        for r in a.views.get(view).unwrap_or(&empty) {
            *count.entry(r.as_str()).or_default() += 1;
        }
        for r in b.views.get(view).unwrap_or(&empty) {
            *count.entry(r.as_str()).or_default() -= 1;
        }
        for (row, n) in count {
            let (label, times) = if n > 0 { (labels.0, n) } else { (labels.1, -n) };
            for _ in 0..times {
                out.push(format!("{view}: only in {label}: {row}"));
            }
        }
    }
    out
}
