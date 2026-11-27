//! Writes the parts of weft-impl that follow from the generated code (`target/generated/`), so
//! no list in this crate is kept by hand:
//!
//! - the storage port on SQLite: one `…Storage` implementation on `Ports` per entity the
//!   generated memory store (`weft-server/src/memory.rs`) keeps, its table named by the entity's
//!   qualified name, and the `Row` enum `seed` writes through;
//! - the codec of every type a stored row reaches (`weft-types/src/*.rs`), from which the rows
//!   are written to and read from their tables, and of the events the hand-run bindings hold
//!   ([`HELD_EVENTS`]);
//! - the behaviour bundle: every file of `src/behaviours/` is a module whose name is the method
//!   of the obligation it implements (`create_tenant.rs` implements `CreateTenantBehavior`), and
//!   every obligation without a file answers the generated stub, `UnmetObligation`. A later story
//!   adds its own files and edits no shared list.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let manifest =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let crates = manifest.join("../../../../target/generated/crates");
    let memory_path = crates.join("weft-server/src/memory.rs");
    let types_dir = crates.join("weft-types/src");
    let behaviours_dir = manifest.join("src/behaviours");
    for path in [&memory_path, &types_dir, &behaviours_dir] {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let memory = read(&memory_path);
    let storages = storages(&memory);
    let obligations = obligations(&memory);
    let behaviours = behaviour_files(&behaviours_dir);
    for name in &behaviours {
        assert!(
            obligations.iter().any(|(method, _)| method == name),
            "src/behaviours/{name}.rs names no obligation: its file name must be the method of an obligation trait of the generated code"
        );
    }
    let types = types(&types_dir);

    // Two files: the generated code, which src/lib.rs includes inside a module clippy does not
    // lint, and the modules of the hand-written behaviours, which it includes at the crate root.
    let mut generated = String::new();
    generated.push_str("// Written by build.rs from target/generated; see its header.\n\n");
    write_storage(&mut generated, &storages);
    write_codecs(&mut generated, &types, &storages);
    write_stubs(&mut generated, &obligations, &behaviours);
    let mut by_hand = String::new();
    by_hand.push_str("// Written by build.rs: the modules of src/behaviours/.\n\n");
    write_behaviours(&mut by_hand, &behaviours, &behaviours_dir);

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    for (name, text) in [
        ("weft_impl_generated.rs", generated),
        ("weft_impl_behaviours.rs", by_hand),
    ] {
        let target = out_dir.join(name);
        std::fs::write(&target, text)
            .unwrap_or_else(|error| panic!("cannot write {}: {error}", target.display()));
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "cannot read {}: {error}; tools/synth.sh writes target/generated/",
            path.display()
        )
    })
}

/// One entity the generated behaviours read or write.
struct Storage {
    /// `weft.access.ApiToken`.
    entity: String,
    /// `weft_types::behaviour::ApiTokenStorage`.
    trait_path: String,
    /// `ApiToken`: the trait's name without `Storage`, the variant of `Row`.
    variant: String,
    /// `weft_types::shared::ApiTokenId`.
    identity: String,
    /// `weft_types::access::ApiTokenSnapshot`.
    snapshot: String,
    /// `api_token_id`: the identity field of the row's data.
    field: String,
    /// Whether the trait declares `list` (a storage no behaviour lists declares none).
    list: bool,
}

/// The entities of the generated memory store, in its order.
fn storages(memory: &str) -> Vec<Storage> {
    let lines: Vec<&str> = memory.lines().collect();
    let mut entity = None;
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if let Some(rest) = line.strip_prefix("/// Ephemeral storage of `") {
            entity = rest.split('`').next().map(str::to_owned);
            continue;
        }
        let Some(rest) = line.strip_prefix("impl weft_types::behaviour::") else {
            continue;
        };
        let Some((name, target)) = rest.split_once(" for ") else {
            continue;
        };
        if !name.ends_with("Storage")
            || !(target.starts_with("Memory") && target.ends_with("Storage {"))
        {
            continue;
        }
        let get = lines.get(index + 1).copied().unwrap_or_default();
        let identity = between(get, "identity: &", ") -> Option<");
        let snapshot = between(get, ") -> Option<", "> {");
        let field = between(get, "(&row.data.", ")");
        let key = between(get, "let key = ", "(identity)");
        let key_line = lines
            .iter()
            .find(|line| line.starts_with(&format!("fn {key}(")))
            .unwrap_or_else(|| panic!("memory.rs declares no {key}"));
        assert!(
            key_line.contains("MemoryKey::Text("),
            "the memory store orders the rows of {name} by a key that is not text ({key_line}); src/storage/mod.rs orders by the key's text, so it needs this key's order first"
        );
        let list = lines[index + 1..]
            .iter()
            .take_while(|line| **line != "}")
            .any(|line| line.starts_with("fn list("));
        found.push(Storage {
            entity: entity
                .take()
                .unwrap_or_else(|| panic!("no entity named before {line}")),
            trait_path: format!("weft_types::behaviour::{name}"),
            variant: name.trim_end_matches("Storage").to_owned(),
            identity,
            snapshot,
            field,
            list,
        });
    }
    assert!(!found.is_empty(), "memory.rs declares no storage");
    found
}

fn between(text: &str, start: &str, end: &str) -> String {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("no `{start}` in {text}"))
        + start.len();
    let to = text[from..]
        .find(end)
        .unwrap_or_else(|| panic!("no `{end}` after `{start}` in {text}"));
    text[from..from + to].to_owned()
}

/// Every obligation the memory store forwards to the generated stub: the method and the
/// implementation's text.
fn obligations(memory: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = memory.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(rest) = line.strip_prefix("impl weft_types::") else {
            continue;
        };
        if !line.ends_with(" for MemoryPorts {") || rest.starts_with("behaviour::") {
            continue;
        }
        let body = lines.get(index + 1).copied().unwrap_or_default();
        assert_eq!(
            lines.get(index + 2).copied(),
            Some("}"),
            "an obligation impl of more than one line: {line}"
        );
        let method = between(body, "fn ", "(");
        let text = format!(
            "{}\n{body}\n}}\n",
            line.replace(" for MemoryPorts {", " for crate::Ports {")
        );
        found.push((method, text));
    }
    assert!(!found.is_empty(), "memory.rs forwards no obligation");
    found
}

/// The stems of the `.rs` files of `src/behaviours/`.
fn behaviour_files(dir: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
    {
        let path = entry.expect("a directory entry").path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("a UTF-8 file name");
            names.insert(stem.to_owned());
        }
    }
    names
}

fn write_storage(out: &mut String, storages: &[Storage]) {
    out.push_str("/// The qualified name of every entity with a table, one table each.\npub const ENTITIES: &[&str] = &[\n");
    for storage in storages {
        let _ = writeln!(out, "    {:?},", storage.entity);
    }
    out.push_str("];\n\n");

    out.push_str("/// A row of any entity, as `seed` writes it through the storage port.\n#[derive(Debug, Clone)]\npub enum Row {\n");
    for storage in storages {
        let _ = writeln!(
            out,
            "    /// A row of `{}`.\n    {}({}),",
            storage.entity, storage.variant, storage.snapshot
        );
    }
    out.push_str("}\n\n");

    out.push_str("impl Ports {\n    /// Writes `row` into its table through the storage port, replacing a row of the same identity.\n    pub fn put_row(&mut self, row: Row) {\n        match row {\n");
    for storage in storages {
        let _ = writeln!(
            out,
            "            Row::{}(row) => {}::put(self, row),",
            storage.variant, storage.trait_path
        );
    }
    out.push_str("        }\n    }\n}\n\n");

    for storage in storages {
        let Storage {
            entity,
            trait_path,
            identity,
            snapshot,
            field,
            list,
            ..
        } = storage;
        let _ = write!(
            out,
            "impl {trait_path} for Ports {{\n\
             fn get(&self, identity: &{identity}) -> Option<{snapshot}> {{ self.store().get({entity:?}, identity) }}\n\
             fn put(&mut self, snapshot: {snapshot}) {{ self.store().put({entity:?}, &snapshot.data.{field}, &snapshot) }}\n\
             fn delete(&mut self, identity: &{identity}) {{ self.store().delete({entity:?}, identity) }}\n"
        );
        if *list {
            let _ = writeln!(
                out,
                "fn list(&self) -> Vec<{snapshot}> {{ self.store().list({entity:?}) }}"
            );
        }
        out.push_str("}\n\n");
    }
}

fn write_behaviours(out: &mut String, behaviours: &BTreeSet<String>, dir: &Path) {
    out.push_str("/// The obligations written by hand, one file each.\nmod behaviours {\n");
    for name in behaviours {
        let path = dir.join(format!("{name}.rs"));
        let _ = writeln!(
            out,
            "    #[path = {:?}]\n    mod {name};",
            path.display().to_string()
        );
    }
    out.push_str("}\n");
}

fn write_stubs(out: &mut String, obligations: &[(String, String)], behaviours: &BTreeSet<String>) {
    out.push_str(
        "// Every obligation without a file of src/behaviours/ answers the generated stub.\n",
    );
    for (method, text) in obligations {
        if !behaviours.contains(method) {
            out.push_str(text);
        }
    }
}

/// A type declared at the top level of a module of weft-types.
enum Shape {
    /// `pub struct Name;`
    Unit,
    /// `pub struct Name(pub A, pub B);`: the count of fields.
    Tuple(usize, Vec<String>),
    /// `pub struct Name { pub field: Type, … }`
    Named(Vec<(String, String)>),
    /// `pub enum Name { … }`
    Enum(Vec<(String, Variant)>),
    /// A generic type, which no stored row holds.
    Generic,
}

enum Variant {
    Unit,
    Tuple(Vec<String>),
    Named(Vec<(String, String)>),
}

/// `(module, name)` of every top-level struct and enum of weft-types.
fn types(dir: &Path) -> BTreeMap<(String, String), Shape> {
    let mut types = BTreeMap::new();
    for entry in std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
    {
        let path = entry.expect("a directory entry").path();
        let Some(module) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if module == "lib"
            || path.extension().and_then(|extension| extension.to_str()) != Some("rs")
        {
            continue;
        }
        let text = read(&path);
        let lines: Vec<&str> = text.lines().collect();
        let mut index = 0;
        while index < lines.len() {
            let line = lines[index];
            index += 1;
            let (is_enum, rest) = if let Some(rest) = line.strip_prefix("pub struct ") {
                (false, rest)
            } else if let Some(rest) = line.strip_prefix("pub enum ") {
                (true, rest)
            } else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let after = &rest[name.len()..];
            let body = |index: &mut usize| {
                let mut body = Vec::new();
                while *index < lines.len() && lines[*index] != "}" {
                    body.push(lines[*index]);
                    *index += 1;
                }
                *index += 1;
                body
            };
            let shape = if after.trim() == "{}" {
                if is_enum {
                    Shape::Enum(Vec::new())
                } else {
                    Shape::Named(Vec::new())
                }
            } else if after.starts_with('<') {
                if after.trim_end().ends_with('{') {
                    body(&mut index);
                }
                Shape::Generic
            } else if after.starts_with(';') {
                Shape::Unit
            } else if let Some(inner) = after.strip_prefix('(') {
                let inner = inner.trim_end().trim_end_matches(';').trim_end_matches(')');
                let fields: Vec<String> = split_top(inner)
                    .into_iter()
                    .map(|field| field.trim().trim_start_matches("pub ").trim().to_owned())
                    .collect();
                Shape::Tuple(fields.len(), fields)
            } else if is_enum {
                Shape::Enum(variants(&body(&mut index), &name))
            } else {
                Shape::Named(fields(&body(&mut index), &name))
            };
            types.insert((module.to_owned(), name), shape);
        }
    }
    types
}

fn code(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    (!trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("#["))
        .then_some(trimmed)
}

fn fields(body: &[&str], name: &str) -> Vec<(String, String)> {
    body.iter()
        .filter_map(|line| code(line))
        .map(|line| {
            let line = line.trim_start_matches("pub ").trim_end_matches(',');
            let (field, ty) = line
                .split_once(':')
                .unwrap_or_else(|| panic!("struct {name}: not a field: {line}"));
            (field.trim().to_owned(), ty.trim().to_owned())
        })
        .collect()
}

fn variants(body: &[&str], name: &str) -> Vec<(String, Variant)> {
    let mut variants = Vec::new();
    let mut index = 0;
    while index < body.len() {
        let line = body[index];
        index += 1;
        let Some(trimmed) = code(line) else { continue };
        let variant: String = trimmed
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let after = trimmed[variant.len()..].trim();
        let shape = if after == "," || after.is_empty() {
            Variant::Unit
        } else if let Some(inner) = after.strip_prefix('(') {
            let inner = inner.trim_end_matches(',').trim_end_matches(')');
            Variant::Tuple(
                split_top(inner)
                    .into_iter()
                    .map(|ty| ty.trim().to_owned())
                    .collect(),
            )
        } else if after == "{" {
            let mut lines = Vec::new();
            while index < body.len() && body[index].trim() != "}," && body[index].trim() != "}" {
                lines.push(body[index]);
                index += 1;
            }
            index += 1;
            Variant::Named(fields(&lines, name))
        } else {
            panic!("enum {name}: not a variant: {trimmed}")
        };
        variants.push((variant, shape));
    }
    variants
}

/// Splits on the commas outside any `<…>` or `(…)`.
fn split_top(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut current = String::new();
    for c in text.chars() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                if !current.trim().is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// The types weft-types declares that `ty`, written in module `module`, names.
fn named(
    ty: &str,
    module: &str,
    types: &BTreeMap<(String, String), Shape>,
) -> Vec<(String, String)> {
    const BUILTIN: &[&str] = &[
        "Option", "Vec", "String", "Box", "bool", "u8", "i64", "u64", "i32", "u32", "usize", "f64",
    ];
    let mut found = Vec::new();
    for token in ty.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':')) {
        if token.is_empty() {
            continue;
        }
        if let Some(path) = token.strip_prefix("crate::") {
            let parts: Vec<&str> = path.split("::").collect();
            assert!(parts.len() == 2, "an unexpected path {token} in {ty}");
            found.push((parts[0].to_owned(), parts[1].to_owned()));
        } else if token.contains("::") || BUILTIN.contains(&token) {
            continue;
        } else {
            let key = (module.to_owned(), token.to_owned());
            assert!(
                types.contains_key(&key),
                "{token} in {ty} is no type of module {module}"
            );
            found.push(key);
        }
    }
    found
}

/// Types whose codec is written by hand in `src/storage/codec.rs`.
const BY_HAND: &[(&str, &str)] = &[("primitives", "Binary64")];

/// Types stored beside the rows: the events the hand-run bindings of `src/bindings/` hold for
/// another attempt (table `weft.impl.Held`).
const HELD_EVENTS: &[(&str, &str)] = &[
    ("workloads", "ReplicaRequested"),
    ("workloads", "InstanceCreated"),
];

fn write_codecs(out: &mut String, types: &BTreeMap<(String, String), Shape>, storages: &[Storage]) {
    let mut queue: VecDeque<(String, String)> = VecDeque::new();
    for storage in storages {
        for ty in [&storage.snapshot, &storage.identity] {
            let path = ty
                .strip_prefix("weft_types::")
                .unwrap_or_else(|| panic!("{ty} is not a weft_types path"));
            let (module, name) = path.split_once("::").expect("module::Name");
            queue.push_back((module.to_owned(), name.to_owned()));
        }
    }
    for (module, name) in HELD_EVENTS {
        queue.push_back(((*module).to_owned(), (*name).to_owned()));
    }
    let mut seen = BTreeSet::new();
    while let Some(key) = queue.pop_front() {
        if !seen.insert(key.clone()) || BY_HAND.contains(&(key.0.as_str(), key.1.as_str())) {
            continue;
        }
        let (module, name) = &key;
        let shape = types.get(&key).unwrap_or_else(|| {
            panic!("weft_types::{module}::{name} is not declared at the top level of weft-types")
        });
        let mut refs = Vec::new();
        match shape {
            Shape::Unit => {}
            Shape::Tuple(_, fields) => fields
                .iter()
                .for_each(|ty| refs.extend(named(ty, module, types))),
            Shape::Named(fields) => fields
                .iter()
                .for_each(|(_, ty)| refs.extend(named(ty, module, types))),
            Shape::Enum(variants) => {
                for (_, variant) in variants {
                    match variant {
                        Variant::Unit => {}
                        Variant::Tuple(fields) => fields
                            .iter()
                            .for_each(|ty| refs.extend(named(ty, module, types))),
                        Variant::Named(fields) => fields
                            .iter()
                            .for_each(|(_, ty)| refs.extend(named(ty, module, types))),
                    }
                }
            }
            Shape::Generic => {
                panic!("weft_types::{module}::{name} is generic; a stored row cannot hold it")
            }
        }
        queue.extend(refs);
        write_codec(out, module, name, shape);
    }
}

fn write_codec(out: &mut String, module: &str, name: &str, shape: &Shape) {
    let ty = format!("weft_types::{module}::{name}");
    let (encode, decode) = match shape {
        Shape::Unit => (
            "Value::Null".to_owned(),
            format!("{{ let _ = value; Ok({ty}) }}"),
        ),
        Shape::Tuple(1, _) => (
            "Codec::encode(&self.0)".to_owned(),
            format!("Ok({ty}(Codec::decode(value).map_err(|error| at({name:?}, error))?))"),
        ),
        Shape::Tuple(count, _) => {
            let items: Vec<String> = (0..*count)
                .map(|index| format!("Codec::encode(&self.{index})"))
                .collect();
            let fields: Vec<String> = (0..*count)
                .map(|index| format!("Codec::decode(item(value, {index})?)?"))
                .collect();
            (
                format!("Value::Array(vec![{}])", items.join(", ")),
                format!("Ok({ty}({}))", fields.join(", ")),
            )
        }
        Shape::Named(fields) => {
            let members: Vec<String> = fields
                .iter()
                .map(|(field, _)| format!("({field:?}.to_owned(), Codec::encode(&self.{field}))"))
                .collect();
            let decoded: Vec<String> = fields
                .iter()
                .map(|(field, _)| format!("{field}: Codec::decode(member(value, {field:?})?).map_err(|error| at({field:?}, error))?"))
                .collect();
            (
                format!(
                    "Value::Object(vec![{}].into_iter().collect())",
                    members.join(", ")
                ),
                format!("Ok({ty} {{ {} }})", decoded.join(", ")),
            )
        }
        Shape::Enum(variants) => {
            let mut encode = String::from("match self {\n");
            let mut decode = String::from(
                "let (variant, inner) = variant(value)?;\nlet _ = inner;\nmatch variant {\n",
            );
            for (variant, shape) in variants {
                match shape {
                    Variant::Unit => {
                        let _ = writeln!(
                            encode,
                            "{ty}::{variant} => Value::String({variant:?}.to_owned()),"
                        );
                        let _ = writeln!(decode, "{variant:?} => Ok({ty}::{variant}),");
                    }
                    Variant::Tuple(fields) => {
                        let names: Vec<String> =
                            (0..fields.len()).map(|index| format!("f{index}")).collect();
                        let items: Vec<String> = names
                            .iter()
                            .map(|field| format!("Codec::encode({field})"))
                            .collect();
                        let decoded: Vec<String> = (0..fields.len())
                            .map(|index| format!("Codec::decode(item(inner.ok_or_else(|| \"no value\".to_owned())?, {index})?)?"))
                            .collect();
                        let _ = writeln!(
                            encode,
                            "{ty}::{variant}({}) => tagged({variant:?}, Value::Array(vec![{}])),",
                            names.join(", "),
                            items.join(", ")
                        );
                        let _ = writeln!(
                            decode,
                            "{variant:?} => Ok({ty}::{variant}({})),",
                            decoded.join(", ")
                        );
                    }
                    Variant::Named(fields) => {
                        let names: Vec<&str> =
                            fields.iter().map(|(field, _)| field.as_str()).collect();
                        let members: Vec<String> = names
                            .iter()
                            .map(|field| format!("({field:?}.to_owned(), Codec::encode({field}))"))
                            .collect();
                        let decoded: Vec<String> = names
                            .iter()
                            .map(|field| format!("{field}: Codec::decode(member(inner.ok_or_else(|| \"no value\".to_owned())?, {field:?})?)?"))
                            .collect();
                        let _ = writeln!(
                            encode,
                            "{ty}::{variant} {{ {} }} => tagged({variant:?}, Value::Object(vec![{}].into_iter().collect())),",
                            names.join(", "),
                            members.join(", ")
                        );
                        let _ = writeln!(
                            decode,
                            "{variant:?} => Ok({ty}::{variant} {{ {} }}),",
                            decoded.join(", ")
                        );
                    }
                }
            }
            encode.push('}');
            let _ = write!(
                decode,
                "other => Err(format!(\"{name}: no variant {{other}}\")),\n}}"
            );
            (encode, decode)
        }
        Shape::Generic => unreachable!("refused in write_codecs"),
    };
    let _ = write!(
        out,
        "impl crate::storage::codec::Codec for {ty} {{\n\
         #[allow(unused_imports)]\n\
         fn encode(&self) -> serde_json::Value {{ use crate::storage::codec::*; use serde_json::Value; {encode} }}\n\
         #[allow(unused_imports)]\n\
         fn decode(value: &serde_json::Value) -> Result<Self, String> {{ use crate::storage::codec::*; {decode} }}\n\
         }}\n\n"
    );
}
