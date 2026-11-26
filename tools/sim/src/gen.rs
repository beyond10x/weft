//! Input values: the witnesses synthesis builds for a guard, the interpreter's own minting for a
//! type, and a cache of both shared by every walk of a run.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

use ess_compiler::ir::{EssIr, ResolvedTypeRef};
use ess_conformance::interpret::values;
use ess_conformance::witness::{self, Distinction};
use ess_primitives::node::Node;
use ess_primitives::predicate::Predicate;

use crate::catalog::Catalog;

pub type Input = BTreeMap<String, Node>;

/// Ticks at and above this one are values no interpreter counter reaches in a walk: a "fresh"
/// identity names no row.
pub const FRESH_BASE: u64 = 500_000_000_000;

#[derive(Default)]
pub struct Candidates {
    map: RwLock<HashMap<(usize, Option<usize>), Arc<Vec<Input>>>>,
}

impl Candidates {
    /// Candidate inputs for `cmd`, directed at the input guard of `outcome` where given.
    pub fn get(&self, cat: &Catalog, cmd: usize, outcome: Option<usize>) -> Arc<Vec<Input>> {
        let key = (cmd, outcome);
        if let Some(found) = self.map.read().expect("candidate cache").get(&key) {
            return found.clone();
        }
        let info = &cat.commands[cmd];
        let command = &cat.ir.commands()[&info.name];
        let guards: Vec<&Predicate> = outcome
            .and_then(|o| info.outcomes[o].guard.as_ref())
            .into_iter()
            .collect();
        let mut found = witness::candidates(&cat.ir, command, &guards, Distinction::PLAIN)
            .unwrap_or_default();
        found.truncate(12);
        let found = Arc::new(found);
        self.map
            .write()
            .expect("candidate cache")
            .insert(key, found.clone());
        found
    }

    pub fn len(&self) -> usize {
        self.map.read().map(|m| m.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A value of `t` (its required form), the `tick`-th the interpreter would mint.
pub fn mint(ir: &EssIr, t: &ResolvedTypeRef, tick: u64) -> Option<Node> {
    values::mint(ir, t.required(), tick).ok().flatten()
}

/// Every value of a declared scalar type inside `node`, read by `t`: what a walk can name later.
pub fn harvest(ir: &EssIr, t: &ResolvedTypeRef, node: &Node, out: &mut Vec<(String, Node)>) {
    match (t, node) {
        (_, Node::Null) => {}
        (ResolvedTypeRef::Optional { of }, _) => harvest(ir, of, node, out),
        (ResolvedTypeRef::List { of }, Node::Seq(items)) => {
            for item in items.iter().take(16) {
                harvest(ir, of, item, out);
            }
        }
        (ResolvedTypeRef::Map { value, .. }, Node::Map(items)) => {
            for item in items.values().take(16) {
                harvest(ir, value, item, out);
            }
        }
        (ResolvedTypeRef::Declared { name }, _) => {
            let declared = ir.named_type(name);
            match (&declared.body, node) {
                (ess_compiler::ir::ResolvedBody::Struct { fields, .. }, Node::Map(map)) => {
                    for f in fields {
                        if let Some(v) = map.get(&f.name) {
                            harvest(ir, &f.type_ref, v, out);
                        }
                    }
                }
                (ess_compiler::ir::ResolvedBody::Newtype { .. }, Node::Text(_) | Node::Number(_)) => {
                    out.push((name.name().to_string(), node.clone()));
                }
                _ => {}
            }
        }
        _ => {}
    }
}
