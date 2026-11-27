//! Probe of the generated crates' form: a crate that uses its sibling through the tree's own
//! `[workspace.dependencies]`.

pub use weft_probe_types::ProbeId;

pub fn echo(id: ProbeId) -> ProbeId {
    id
}
