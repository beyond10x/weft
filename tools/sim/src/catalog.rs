//! The compiled model, indexed for a walk: commands with their outcomes, entities with their
//! lifecycles, actors with their grants, and dense ids for coverage counters.

use std::collections::{BTreeSet, HashMap};

use ess_compiler::ir::{
    EssIr, ResolvedBody, ResolvedCondition, ResolvedEffect, ResolvedField, ResolvedInstance,
    ResolvedPayloadValue, ResolvedRelatedTest, ResolvedRelatedVia, ResolvedRowSetTest,
    ResolvedTypeRef,
};
use ess_domain::name::QualifiedName;
use ess_domain::types::Primitive;
use ess_primitives::predicate::{CompareOp, Operand, Predicate};
use ess_primitives::FactValue;

/// An input field a guard compares with a field of the row another input field names:
/// `when_related: {via: input.<via>, predicate: <row_field> != input.<input_field>}`. A walk copies
/// the row's field into the input, so the guard passes.
#[derive(Debug, Clone)]
pub struct Tie {
    pub via: String,
    pub ent: usize,
    pub row_field: String,
    pub input_field: String,
}

/// A row a guard requires to exist (`when_related: {entity, where, exists}` refused when absent):
/// the `where` atoms `<row_field> == input.<input_field>` and the states it admits. A walk picks a
/// live row in one of those states and copies its fields into the input.
#[derive(Debug, Clone)]
pub struct SetTie {
    pub ent: usize,
    pub pairs: Vec<(String, String)>,
    pub states: Vec<usize>,
}

/// The atoms of a predicate that compare a row's field with an input field, with the operator
/// as it reads once every enclosing `not` is applied.
fn compare_atoms(p: &Predicate, negated: bool, out: &mut Vec<(String, String, CompareOp)>) {
    match p {
        Predicate::All(items) | Predicate::Any(items) => {
            for item in items {
                compare_atoms(item, negated, out);
            }
        }
        Predicate::Not(inner) => compare_atoms(inner, !negated, out),
        Predicate::Compare { left: Operand::Fact(a), op, right: Operand::Fact(b), .. } => {
            let input_of = |path: &ess_primitives::FactPath| -> Option<String> {
                let s = path.segments();
                (s.len() == 2 && s[0] == "input").then(|| s[1].clone())
            };
            let row_of = |path: &ess_primitives::FactPath| -> Option<String> {
                let s = path.segments();
                (s.len() == 1 && s[0] != "input").then(|| s[0].clone())
            };
            let pair = match (row_of(a), input_of(b), input_of(a), row_of(b)) {
                (Some(row), Some(input), _, _) | (_, _, Some(input), Some(row)) => Some((row, input)),
                _ => None,
            };
            let op = match (op, negated) {
                (CompareOp::Eq, true) => CompareOp::Ne,
                (CompareOp::Ne, true) => CompareOp::Eq,
                (other, _) => *other,
            };
            if let Some((row, input)) = pair {
                out.push((row, input, op));
            }
        }
        _ => {}
    }
}

/// The states a `where` admits through `state == X` or `{state: [X, Y]}` at its top level.
fn state_atoms(p: &Predicate, ent: &Ent, out: &mut Vec<usize>) {
    let text = |v: &FactValue| match v {
        FactValue::Text(t) => Some(t.clone()),
        _ => None,
    };
    match p {
        Predicate::All(items) => {
            for item in items {
                state_atoms(item, ent, out);
            }
        }
        Predicate::AnyOf { path, values } if path.segments() == ["state"] => {
            out.extend(values.iter().filter_map(text).filter_map(|s| ent.state_index(&s)));
        }
        Predicate::Compare { left: Operand::Fact(path), op: CompareOp::Eq, right: Operand::Literal(v), .. }
            if path.segments() == ["state"] =>
        {
            out.extend(text(v).and_then(|s| ent.state_index(&s)));
        }
        _ => {}
    }
}

#[derive(Debug, Clone)]
pub struct Tr {
    pub name: String,
    pub from: Vec<usize>,
    pub to: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Bool,
    Num,
    Enum,
}

#[derive(Debug)]
pub struct Ent {
    pub name: QualifiedName,
    pub domain: String,
    pub states: Vec<String>,
    pub state_base: usize,
    pub initial: usize,
    pub terminal: Vec<usize>,
    pub transitions: Vec<Tr>,
    pub tr_base: usize,
    pub identity_field: String,
    pub identity_type: Option<QualifiedName>,
    /// Fields a guard reads, of a kind that buckets: Boolean, number, enum.
    pub tracked: Vec<(String, FieldKind)>,
    /// Every stored field, by name.
    pub fields: BTreeSet<String>,
}

impl Ent {
    pub fn state_index(&self, name: &str) -> Option<usize> {
        self.states.iter().position(|s| s == name)
    }
    pub fn transition_index(&self, name: &str) -> Option<usize> {
        self.transitions.iter().position(|t| t.name == name)
    }
}

#[derive(Debug, Clone)]
pub enum InstanceSrc {
    Input(String),
    Event(QualifiedName, String),
}

#[derive(Debug, Clone)]
pub enum SetSrc {
    Input(String),
    Literal(String),
    Increment(String),
    Cleared,
    Other,
}

#[derive(Debug, Clone)]
pub enum Eff {
    Creates { into: usize },
    Moves { tr: usize, to: usize },
    Updates,
    Preserves,
    Deletes,
}

#[derive(Debug, Clone)]
pub struct Subj {
    pub ent: usize,
    pub effect: Eff,
    pub instance: InstanceSrc,
    pub sets: Vec<(String, SetSrc)>,
}

#[derive(Debug)]
pub struct Out {
    pub name: String,
    pub kind: &'static str,
    pub error: bool,
    pub external: bool,
    pub guard: Option<Predicate>,
    pub subject: Option<Subj>,
    /// Transitions a set subject (`instances:`) or an `affects:` takes: (entity, transition).
    pub set_moves: Vec<(usize, usize)>,
    pub gid: usize,
    /// The guard reads a stored row beyond its existence: a state, a field predicate, a row set.
    /// Not an input-only `when:`, an existence check or the default.
    pub reads_rows: bool,
}

#[derive(Debug)]
pub struct Cmd {
    pub name: QualifiedName,
    pub domain: String,
    pub input: Vec<ResolvedField>,
    pub response: Vec<ResolvedField>,
    pub actors: Vec<usize>,
    pub outcomes: Vec<Out>,
    /// Input fields that name an existing row (an entity identity type), required.
    pub id_inputs: Vec<(String, String)>,
    /// Input fields a `creates:` names the new identity in.
    pub create_inputs: BTreeSet<String>,
    pub creates: bool,
    /// Identity types of the entities its outcomes create.
    pub creates_types: Vec<String>,
    /// Input fields of an entity identity type (Optional ones too), with the entity.
    pub id_ents: Vec<(String, usize)>,
    /// Input fields the guards tie to a field of a row another input names.
    pub ties: Vec<Tie>,
    /// Rows the guards require to exist, tied to input fields.
    pub set_ties: Vec<SetTie>,
}

#[derive(Debug)]
pub struct Actor {
    pub name: QualifiedName,
    pub attributes: Vec<ResolvedField>,
}

#[derive(Debug, Clone)]
pub struct BindingInfo {
    pub name: ess_domain::binding::BindingName,
    pub command: usize,
}

pub struct Catalog {
    pub ir: EssIr,
    pub commands: Vec<Cmd>,
    pub cmd_index: HashMap<QualifiedName, usize>,
    pub entities: Vec<Ent>,
    pub ent_index: HashMap<QualifiedName, usize>,
    pub actors: Vec<Actor>,
    pub actor_index: HashMap<QualifiedName, usize>,
    pub outcomes_total: usize,
    pub states_total: usize,
    pub transitions_total: usize,
    /// (command, outcome) of every outcome gid.
    pub outcome_of: Vec<(usize, usize)>,
    pub identity_types: HashMap<QualifiedName, usize>,
    pub bindings_by_event: HashMap<QualifiedName, Vec<BindingInfo>>,
    pub domains: Vec<String>,
    /// How many commands need a row of each identity type.
    pub demand: HashMap<String, usize>,
    /// Bindings the spec retries without bound that weft-sim bounded (`explore --retry-bound`), and
    /// the bound; 0 when none was set.
    pub retry_bounded: BTreeSet<String>,
    pub retry_bound: u32,
}

fn condition_kind(c: &ResolvedCondition) -> &'static str {
    match c {
        ResolvedCondition::SubjectField { .. } => "subject_field",
        ResolvedCondition::SubjectPredicate { .. } => "subject_predicate",
        ResolvedCondition::Related { .. } => "related",
        ResolvedCondition::RelatedSet { .. } => "related_set",
        ResolvedCondition::When { .. } => "when",
        ResolvedCondition::SubjectState { .. } => "subject_state",
        ResolvedCondition::StateChange { .. } => "state_change",
        ResolvedCondition::Otherwise => "otherwise",
        ResolvedCondition::ExternalWhen { .. } => "external_when",
        ResolvedCondition::External { .. } => "external",
        ResolvedCondition::ExternalSubject { .. } => "external_subject",
        ResolvedCondition::WrongState => "wrong_state",
        ResolvedCondition::UnknownInstance => "unknown_instance",
        ResolvedCondition::InputAbsent => "input_absent",
        ResolvedCondition::ExistingInstance => "existing_instance",
    }
}

fn input_guard(c: &ResolvedCondition) -> Option<Predicate> {
    match c {
        ResolvedCondition::When { predicate } | ResolvedCondition::ExternalWhen { predicate, .. } => {
            Some(predicate.clone())
        }
        ResolvedCondition::SubjectPredicate { input, .. }
        | ResolvedCondition::Related { input, .. }
        | ResolvedCondition::RelatedSet { input, .. }
        | ResolvedCondition::ExternalSubject { input, .. } => input.clone(),
        ResolvedCondition::SubjectField { predicate, .. }
        | ResolvedCondition::SubjectState { predicate, .. }
        | ResolvedCondition::StateChange { predicate, .. } => predicate.clone(),
        _ => None,
    }
}

/// Text of every guard that reads the subject row, to find the fields guards read.
fn subject_guard_text(c: &ResolvedCondition) -> Option<String> {
    match c {
        ResolvedCondition::SubjectField { field, predicate, .. } => Some(format!(
            "{field} {}",
            predicate.as_ref().map(ToString::to_string).unwrap_or_default()
        )),
        ResolvedCondition::SubjectPredicate { predicate, .. }
        | ResolvedCondition::ExternalSubject { predicate, .. } => Some(predicate.to_string()),
        _ => None,
    }
}

pub fn field_kind(ir: &EssIr, t: &ResolvedTypeRef) -> Option<FieldKind> {
    let mut current = t.required().clone();
    for _ in 0..40 {
        match current {
            ResolvedTypeRef::Primitive { name } => {
                return match name {
                    Primitive::Boolean => Some(FieldKind::Bool),
                    Primitive::Integer | Primitive::Decimal | Primitive::Binary64 => {
                        Some(FieldKind::Num)
                    }
                    _ => None,
                }
            }
            ResolvedTypeRef::Declared { ref name } => match &ir.named_type(name).body {
                ResolvedBody::Newtype { of, .. } => current = of.required().clone(),
                ResolvedBody::Enum { .. } => return Some(FieldKind::Enum),
                _ => return None,
            },
            _ => return None,
        }
    }
    None
}

impl Catalog {
    pub fn new(ir: EssIr) -> Self {
        let mut entities = Vec::new();
        let mut ent_index = HashMap::new();
        let mut identity_types = HashMap::new();
        let mut state_base = 0;
        let mut tr_base = 0;
        let mut domains: BTreeSet<String> = BTreeSet::new();
        for (name, entity) in ir.entities() {
            let states: Vec<String> = entity.lifecycle.states.iter().map(ToString::to_string).collect();
            let index_of = |s: &ess_domain::entity::StateName| {
                states.iter().position(|x| *x == s.to_string()).unwrap_or(0)
            };
            let transitions: Vec<Tr> = entity
                .lifecycle
                .transitions
                .iter()
                .map(|t| Tr {
                    name: t.name.clone(),
                    from: t.from.iter().map(index_of).collect(),
                    to: index_of(&t.to),
                })
                .collect();
            let identity_type = entity.identity.type_ref.declared().map(|h| h.name().clone());
            if let Some(t) = &identity_type {
                identity_types.insert(t.clone(), entities.len());
            }
            let domain = entity.domain.name().to_string();
            domains.insert(domain.clone());
            ent_index.insert(name.clone(), entities.len());
            entities.push(Ent {
                name: name.clone(),
                domain,
                initial: index_of(&entity.lifecycle.initial),
                terminal: entity.lifecycle.terminal.iter().map(index_of).collect(),
                state_base,
                tr_base,
                identity_field: entity.identity.name.clone(),
                identity_type,
                tracked: Vec::new(),
                fields: entity.fields.iter().map(|f| f.name.clone()).collect(),
                states,
                transitions,
            });
            state_base += entities.last().map_or(0, |e| e.states.len());
            tr_base += entities.last().map_or(0, |e| e.transitions.len());
        }

        let mut actors = Vec::new();
        let mut actor_index = HashMap::new();
        for (name, actor) in ir.actors() {
            actor_index.insert(name.clone(), actors.len());
            actors.push(Actor {
                name: name.clone(),
                attributes: actor.attributes.clone(),
            });
        }

        let mut guard_text: HashMap<usize, String> = HashMap::new();
        let mut commands = Vec::new();
        let mut cmd_index = HashMap::new();
        let mut outcome_of = Vec::new();
        let mut gid = 0;
        for (name, command) in ir.commands() {
            let ci = commands.len();
            let mut granted: Vec<usize> = ir
                .actors()
                .iter()
                .filter(|(_, a)| a.may.iter().any(|h| h.name() == name))
                .filter_map(|(n, _)| actor_index.get(n).copied())
                .collect();
            granted.sort_unstable();
            let mut outcomes = Vec::new();
            let mut create_inputs = BTreeSet::new();
            let mut creates = false;
            for (oi, outcome) in command.outcomes.iter().enumerate() {
                let subject = outcome.subject.as_ref().and_then(|s| {
                    let ent = *ent_index.get(s.entity.name())?;
                    let e = &entities[ent];
                    let effect = match &s.effect {
                        ResolvedEffect::Creates => {
                            creates = true;
                            let into = s
                                .into
                                .as_ref()
                                .and_then(|st| e.state_index(&st.to_string()))
                                .unwrap_or(e.initial);
                            Eff::Creates { into }
                        }
                        ResolvedEffect::Moves { transition } => {
                            let tr = e.transition_index(&transition.name).unwrap_or(0);
                            let to = e.state_index(&transition.to.to_string()).unwrap_or(0);
                            Eff::Moves { tr, to }
                        }
                        ResolvedEffect::Updates => Eff::Updates,
                        ResolvedEffect::Preserves => Eff::Preserves,
                        ResolvedEffect::Deletes => Eff::Deletes,
                    };
                    let instance = match &s.instance {
                        ResolvedInstance::Supplied { field } => {
                            if matches!(effect, Eff::Creates { .. }) {
                                create_inputs.insert(field.name.clone());
                            }
                            InstanceSrc::Input(field.name.clone())
                        }
                        ResolvedInstance::Observed { event, field } => {
                            InstanceSrc::Event(event.name().clone(), field.name.clone())
                        }
                    };
                    let sets = outcome
                        .sets
                        .iter()
                        .map(|f| {
                            let src = match &f.value {
                                ResolvedPayloadValue::InputField { field, .. } => {
                                    SetSrc::Input(field.clone())
                                }
                                ResolvedPayloadValue::Literal { value } => {
                                    SetSrc::Literal(value.clone())
                                }
                                ResolvedPayloadValue::Increment { by } => {
                                    SetSrc::Increment(by.clone())
                                }
                                ResolvedPayloadValue::Cleared => SetSrc::Cleared,
                                _ => SetSrc::Other,
                            };
                            (f.target.clone(), src)
                        })
                        .collect();
                    if let Some(text) = subject_guard_text(&outcome.condition) {
                        let entry = guard_text.entry(ent).or_default();
                        entry.push(' ');
                        entry.push_str(&text);
                    }
                    Some(Subj {
                        ent,
                        effect,
                        instance,
                        sets,
                    })
                });
                let mut set_moves = Vec::new();
                if let Some(set) = &outcome.instances {
                    if let (Some(ent), Some(t)) =
                        (ent_index.get(set.entity.name()), set.effect.transition())
                    {
                        if let Some(tr) = entities[*ent].transition_index(&t.name) {
                            set_moves.push((*ent, tr));
                        }
                    }
                }
                for affect in &outcome.affects {
                    if let (Some(ent), Some(t)) = (ent_index.get(affect.entity.name()), &affect.moves)
                    {
                        if let Some(tr) = entities[*ent].transition_index(&t.name) {
                            set_moves.push((*ent, tr));
                        }
                    }
                }
                outcomes.push(Out {
                    name: outcome.name.to_string(),
                    kind: condition_kind(&outcome.condition),
                    error: outcome.error.is_some(),
                    external: matches!(
                        outcome.condition,
                        ResolvedCondition::External { .. }
                            | ResolvedCondition::ExternalWhen { .. }
                            | ResolvedCondition::ExternalSubject { .. }
                    ),
                    guard: input_guard(&outcome.condition),
                    subject,
                    set_moves,
                    gid,
                    reads_rows: match &outcome.condition {
                        ResolvedCondition::Related { test: ResolvedRelatedTest::Absent, also, .. } => !also.is_empty(),
                        ResolvedCondition::Related { .. }
                        | ResolvedCondition::RelatedSet { .. }
                        | ResolvedCondition::SubjectField { .. }
                        | ResolvedCondition::SubjectPredicate { .. }
                        | ResolvedCondition::SubjectState { .. }
                        | ResolvedCondition::StateChange { .. }
                        | ResolvedCondition::ExternalSubject { .. }
                        | ResolvedCondition::WrongState => true,
                        _ => false,
                    },
                });
                outcome_of.push((ci, oi));
                gid += 1;
            }
            let id_inputs = command
                .input
                .iter()
                .filter(|f| !f.type_ref.is_optional())
                .filter_map(|f| {
                    let t = f.type_ref.declared()?.name().clone();
                    identity_types.contains_key(&t).then(|| (f.name.clone(), t.to_string()))
                })
                .filter(|(n, _)| !create_inputs.contains(n))
                .collect();
            let id_ents: Vec<(String, usize)> = command
                .input
                .iter()
                .filter_map(|f| match f.type_ref.required() {
                    ResolvedTypeRef::Declared { name: t } => {
                        identity_types.get(t.name()).map(|e| (f.name.clone(), *e))
                    }
                    _ => None,
                })
                .collect();
            let input_names: BTreeSet<&str> = command.input.iter().map(|f| f.name.as_str()).collect();
            // The input field naming the command's own subject, for guards over its stored fields.
            let subject_via = outcomes.iter().find_map(|o| match &o.subject {
                Some(Subj { ent, instance: InstanceSrc::Input(field), effect, .. })
                    if !matches!(effect, Eff::Creates { .. }) =>
                {
                    Some((field.clone(), *ent))
                }
                _ => None,
            });
            let mut ties: Vec<Tie> = Vec::new();
            let mut set_ties: Vec<SetTie> = Vec::new();
            let add_ties = |via: &str, ent: usize, p: &Predicate, error: bool, ties: &mut Vec<Tie>| {
                let mut atoms = Vec::new();
                compare_atoms(p, false, &mut atoms);
                // A refusal taken when the fields differ, or an acceptance taken when they agree.
                let wanted = if error { CompareOp::Ne } else { CompareOp::Eq };
                for (row, input, op) in atoms {
                    if op == wanted
                        && input_names.contains(input.as_str())
                        && entities[ent].fields.contains(&row)
                        && !ties.iter().any(|t| t.via == via && t.input_field == input)
                    {
                        ties.push(Tie { via: via.to_owned(), ent, row_field: row, input_field: input });
                    }
                }
            };
            for (outcome, out) in command.outcomes.iter().zip(&outcomes) {
                match &outcome.condition {
                    ResolvedCondition::Related { via: ResolvedRelatedVia::Input { field, .. }, entity, test, also, .. } => {
                        if let (Some(&ent), ResolvedRelatedTest::Holds { predicate }) = (ent_index.get(entity.name()), test) {
                            add_ties(field, ent, predicate, out.error, &mut ties);
                        }
                        for a in also {
                            if let (ResolvedRelatedVia::Input { field, .. }, Some(&ent)) = (&a.via, ent_index.get(a.entity.name())) {
                                add_ties(field, ent, &a.predicate, out.error, &mut ties);
                            }
                        }
                    }
                    ResolvedCondition::SubjectPredicate { predicate, .. }
                    | ResolvedCondition::ExternalSubject { predicate, .. } => {
                        if let Some((field, ent)) = &subject_via {
                            add_ties(field, *ent, predicate, out.error, &mut ties);
                        }
                    }
                    ResolvedCondition::RelatedSet { selection, test: ResolvedRowSetTest::Exists(exists), .. }
                        if *exists != out.error =>
                    {
                        let Some(&ent) = ent_index.get(selection.entity.name()) else { continue };
                        let mut atoms = Vec::new();
                        compare_atoms(&selection.filter, false, &mut atoms);
                        let pairs: Vec<(String, String)> = atoms
                            .into_iter()
                            .filter(|(row, input, op)| {
                                *op == CompareOp::Eq
                                    && input_names.contains(input.as_str())
                                    && entities[ent].fields.contains(row)
                            })
                            .map(|(row, input, _)| (row, input))
                            .collect();
                        if pairs.is_empty() {
                            continue;
                        }
                        let mut states = Vec::new();
                        state_atoms(&selection.filter, &entities[ent], &mut states);
                        set_ties.push(SetTie { ent, pairs, states });
                    }
                    _ => {}
                }
            }
            cmd_index.insert(name.clone(), ci);
            commands.push(Cmd {
                name: name.clone(),
                domain: command.domain.name().to_string(),
                input: command.input.clone(),
                response: command.response.clone(),
                actors: granted,
                outcomes,
                id_inputs,
                create_inputs,
                creates,
                creates_types: Vec::new(),
                id_ents,
                ties,
                set_ties,
            });
        }

        for (ei, ent) in entities.iter_mut().enumerate() {
            let Some(text) = guard_text.get(&ei) else { continue };
            let words: BTreeSet<&str> = text
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|w| !w.is_empty())
                .collect();
            let entity = &ir.entities()[&ent.name];
            for field in &entity.fields {
                if words.contains(field.name.as_str()) {
                    if let Some(kind) = field_kind(&ir, &field.type_ref) {
                        ent.tracked.push((field.name.clone(), kind));
                    }
                }
            }
        }

        let mut demand: HashMap<String, usize> = HashMap::new();
        for c in &commands {
            for (_, t) in &c.id_inputs {
                *demand.entry(t.clone()).or_default() += 1;
            }
        }
        for c in &mut commands {
            let mut types = BTreeSet::new();
            for o in &c.outcomes {
                if let Some(s) = &o.subject {
                    if matches!(s.effect, Eff::Creates { .. }) {
                        if let Some(t) = &entities[s.ent].identity_type {
                            types.insert(t.to_string());
                        }
                    }
                }
            }
            c.creates_types = types.into_iter().collect();
        }

        let mut bindings_by_event: HashMap<QualifiedName, Vec<BindingInfo>> = HashMap::new();
        for (name, binding) in ir.bindings() {
            if let (Some(event), Some(ci)) =
                (binding.cause.event(), cmd_index.get(binding.command.name()))
            {
                bindings_by_event
                    .entry(event.name().clone())
                    .or_default()
                    .push(BindingInfo {
                        name: name.clone(),
                        command: *ci,
                    });
            }
        }

        Self {
            outcomes_total: gid,
            states_total: state_base,
            transitions_total: tr_base,
            ir,
            commands,
            cmd_index,
            entities,
            ent_index,
            actors,
            actor_index,
            outcome_of,
            identity_types,
            bindings_by_event,
            domains: domains.into_iter().collect(),
            demand,
            retry_bounded: BTreeSet::new(),
            retry_bound: 0,
        }
    }

    /// The static half of the page: domains, entities with lifecycles, commands with outcomes.
    pub fn model_json(&self) -> serde_json::Value {
        use serde_json::json;
        let mut domains = Vec::new();
        let mut all: BTreeSet<String> = self.domains.iter().cloned().collect();
        for c in &self.commands {
            all.insert(c.domain.clone());
        }
        for d in &all {
            let entities: Vec<_> = self
                .entities
                .iter()
                .filter(|e| &e.domain == d)
                .map(|e| {
                    json!({
                        "name": e.name.to_string(),
                        "states": e.states,
                        "initial": e.initial,
                        "terminal": e.terminal,
                        "state_base": e.state_base,
                        "tr_base": e.tr_base,
                        "transitions": e.transitions.iter().map(|t| json!({"name": t.name, "from": t.from, "to": t.to})).collect::<Vec<_>>(),
                    })
                })
                .collect();
            let commands: Vec<_> = self
                .commands
                .iter()
                .filter(|c| &c.domain == d)
                .map(|c| {
                    json!({
                        "name": c.name.to_string(),
                        "outcomes": c.outcomes.iter().map(|o| json!({"name": o.name, "kind": o.kind, "error": o.error, "gid": o.gid})).collect::<Vec<_>>(),
                    })
                })
                .collect();
            domains.push(json!({"name": d, "entities": entities, "commands": commands}));
        }
        json!({
            "system": self.ir.system().to_string(),
            "outcomes_total": self.outcomes_total,
            "states_total": self.states_total,
            "transitions_total": self.transitions_total,
            "domains": domains,
        })
    }
}
