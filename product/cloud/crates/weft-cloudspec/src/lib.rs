//! weft's offline reader of a cloudspec weft submission (`cloudspec-weft-submission/1`).
//!
//! cloudspec's `generate weft` writes one JSON document `{format, architecture, binding (null
//! when none), coverage, digests: {architecture, binding}}`; each digest is `sha256:<hex>` of the
//! canonical JSON of its member, `null` included ([`canonical`]). [`Submission::read`] takes the
//! bytes of such a file and answers the accepted submission or the first [`Refusal`] of these
//! checks, in this order: the shape, the format, the architecture digest, the binding digest, the
//! binding's architecture, the binding's roles. It reads no network, starts no process and needs
//! no cloudspec binary.

use std::collections::BTreeSet;
use std::fmt;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// The one format this reader accepts.
pub const FORMAT: &str = "cloudspec-weft-submission/1";

/// The members of a submission; a document has exactly these.
const MEMBERS: [&str; 5] = ["format", "architecture", "binding", "coverage", "digests"];

/// A member of the submission that carries a digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Member {
    /// `architecture`, digest `digests.architecture`.
    Architecture,
    /// `binding`, digest `digests.binding`.
    Binding,
}

impl Member {
    /// The member's key, also its key in `digests`.
    pub fn as_str(self) -> &'static str {
        match self {
            Member::Architecture => "architecture",
            Member::Binding => "binding",
        }
    }
}

/// Why a document is not an accepted submission: the first check it fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Not JSON, not an object with exactly the five members, or `digests` is not an object
    /// holding `architecture` and `binding` with a string value each.
    NotASubmission {
        /// What is missing or malformed.
        reason: String,
    },
    /// `format` is not [`FORMAT`] (another version included).
    UnknownFormat {
        /// The value of `format`.
        found: Value,
    },
    /// The digest of a member's canonical JSON differs from the one `digests` declares.
    DigestMismatch {
        /// The member.
        member: Member,
        /// The digest the document declares.
        declared: String,
        /// The digest of the member's canonical JSON.
        computed: String,
    },
    /// A binding whose `architecture` is not the architecture's `architecture`.
    BindingForOtherArchitecture {
        /// `binding.architecture` (`null` when the binding names none).
        binding: Value,
        /// `architecture.architecture` (`null` when the architecture names none).
        architecture: Value,
    },
    /// A key of `binding.roles` that no part names as `package.runtime.role`.
    BindingRoleNamesNoPart {
        /// The role.
        role: String,
    },
}

impl Refusal {
    /// The refusal's name, the name of its variant.
    pub fn name(&self) -> &'static str {
        match self {
            Refusal::NotASubmission { .. } => "NotASubmission",
            Refusal::UnknownFormat { .. } => "UnknownFormat",
            Refusal::DigestMismatch { .. } => "DigestMismatch",
            Refusal::BindingForOtherArchitecture { .. } => "BindingForOtherArchitecture",
            Refusal::BindingRoleNamesNoPart { .. } => "BindingRoleNamesNoPart",
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NotASubmission { reason } => {
                write!(f, "NotASubmission: not a {FORMAT} document: {reason}")
            }
            Refusal::UnknownFormat { found } => {
                write!(
                    f,
                    "UnknownFormat: format {found}, this reader reads {FORMAT}"
                )
            }
            Refusal::DigestMismatch {
                member,
                declared,
                computed,
            } => write!(
                f,
                "DigestMismatch: digests.{} declares {declared}, the {} is {computed}",
                member.as_str(),
                member.as_str()
            ),
            Refusal::BindingForOtherArchitecture {
                binding,
                architecture,
            } => write!(
                f,
                "BindingForOtherArchitecture: the binding is for architecture {binding}, the \
                 architecture is {architecture}"
            ),
            Refusal::BindingRoleNamesNoPart { role } => write!(
                f,
                "BindingRoleNamesNoPart: the binding's role {role} is named by no part \
                 (package.runtime.role)"
            ),
        }
    }
}

impl std::error::Error for Refusal {}

/// The canonical JSON of a value: object keys sorted by their UTF-8 bytes, no white space,
/// strings and numbers as serde_json writes them. The keys are sorted here, so the text does not
/// depend on whether serde_json keeps insertion order (its `preserve_order` feature).
pub fn canonical(v: &Value) -> String {
    let mut out = String::new();
    write_canonical(v, &mut out);
    out
}

fn write_canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                write_canonical(&m[k.as_str()], out);
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(x, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

/// `sha256:<hex>` of bytes, lower-case hex, as cloudspec writes a digest.
pub fn digest(bytes: &[u8]) -> String {
    let mut out = String::from("sha256:");
    for b in Sha256::digest(bytes) {
        out.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(b & 0x0f), 16).unwrap_or('0'));
    }
    out
}

/// An accepted submission: its members as JSON values, its two digests and the digest of the
/// file's bytes.
#[derive(Debug, Clone)]
pub struct Submission {
    architecture: Value,
    binding: Value,
    coverage: Value,
    architecture_digest: String,
    binding_digest: String,
    source_digest: String,
}

fn not_a_submission(reason: impl Into<String>) -> Refusal {
    Refusal::NotASubmission {
        reason: reason.into(),
    }
}

/// The two declared digests, or why `digests` is not an object with both as strings.
fn declared_digests(digests: &Value) -> Result<(String, String), Refusal> {
    let Value::Object(d) = digests else {
        return Err(not_a_submission("digests is not an object"));
    };
    let one = |m: Member| match d.get(m.as_str()) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(not_a_submission(format!(
            "digests.{} is not a string",
            m.as_str()
        ))),
        None => Err(not_a_submission(format!("digests has no {}", m.as_str()))),
    };
    Ok((one(Member::Architecture)?, one(Member::Binding)?))
}

/// The roles the parts of an architecture name (`parts.<name>.package.runtime.role`).
fn named_roles(architecture: &Value) -> BTreeSet<&str> {
    architecture
        .get("parts")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(Map::values)
        .filter_map(|p| p.pointer("/package/runtime/role").and_then(Value::as_str))
        .collect()
}

impl Submission {
    /// Reads a submission from the bytes of its file: the accepted submission, or the first
    /// refusal of the checks in the order of [`Refusal`]'s variants.
    pub fn read(bytes: &[u8]) -> Result<Self, Refusal> {
        let doc: Value = serde_json::from_slice(bytes)
            .map_err(|e| not_a_submission(format!("not JSON: {e}")))?;
        let Value::Object(mut doc) = doc else {
            return Err(not_a_submission("not a JSON object"));
        };
        for m in MEMBERS {
            if !doc.contains_key(m) {
                return Err(not_a_submission(format!("no member {m}")));
            }
        }
        let mut other: Vec<&String> = doc
            .keys()
            .filter(|k| !MEMBERS.contains(&k.as_str()))
            .collect();
        other.sort();
        if let Some(k) = other.first() {
            return Err(not_a_submission(format!("a member {k} besides the five")));
        }
        let (declared_architecture, declared_binding) = declared_digests(&doc["digests"])?;

        if doc["format"] != FORMAT {
            return Err(Refusal::UnknownFormat {
                found: doc["format"].clone(),
            });
        }

        let architecture = doc.remove("architecture").unwrap_or(Value::Null);
        let binding = doc.remove("binding").unwrap_or(Value::Null);
        let coverage = doc.remove("coverage").unwrap_or(Value::Null);
        let architecture_digest = digest(canonical(&architecture).as_bytes());
        if architecture_digest != declared_architecture {
            return Err(Refusal::DigestMismatch {
                member: Member::Architecture,
                declared: declared_architecture,
                computed: architecture_digest,
            });
        }
        let binding_digest = digest(canonical(&binding).as_bytes());
        if binding_digest != declared_binding {
            return Err(Refusal::DigestMismatch {
                member: Member::Binding,
                declared: declared_binding,
                computed: binding_digest,
            });
        }

        if !binding.is_null() {
            let for_architecture = binding.get("architecture").unwrap_or(&Value::Null);
            let named = architecture.get("architecture").unwrap_or(&Value::Null);
            if !for_architecture.is_string() || for_architecture != named {
                return Err(Refusal::BindingForOtherArchitecture {
                    binding: for_architecture.clone(),
                    architecture: named.clone(),
                });
            }
            let parts_name = named_roles(&architecture);
            let mut roles: Vec<&String> = binding
                .get("roles")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(Map::keys)
                .collect();
            roles.sort();
            if let Some(role) = roles.into_iter().find(|r| !parts_name.contains(r.as_str())) {
                return Err(Refusal::BindingRoleNamesNoPart { role: role.clone() });
            }
        }

        Ok(Submission {
            architecture,
            binding,
            coverage,
            architecture_digest,
            binding_digest,
            source_digest: digest(bytes),
        })
    }

    /// The `cloudspec-architecture/1` document, as JSON.
    pub fn architecture(&self) -> &Value {
        &self.architecture
    }

    /// The binding document, as JSON; `None` when the submission has none (`null`).
    pub fn binding(&self) -> Option<&Value> {
        (!self.binding.is_null()).then_some(&self.binding)
    }

    /// The `cloudspec-coverage/1` report, as JSON.
    pub fn coverage(&self) -> &Value {
        &self.coverage
    }

    /// `digests.architecture`, checked: `sha256:<hex>` of the architecture's canonical JSON.
    pub fn architecture_digest(&self) -> &str {
        &self.architecture_digest
    }

    /// `digests.binding`, checked: `sha256:<hex>` of the binding's canonical JSON (of `null`
    /// when there is none).
    pub fn binding_digest(&self) -> &str {
        &self.binding_digest
    }

    /// `sha256:<hex>` of the file's bytes: the source digest that names the submission.
    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }
}
