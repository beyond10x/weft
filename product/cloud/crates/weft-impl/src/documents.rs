//! The documents the control host keeps beside the rows: the bytes of a submitted file, keyed by
//! its source digest (`weft.deployments.SourceDigest`, `sha256:<hex>` of the bytes).
//!
//! `weft.deployments.SubmitDeploymentSource` carries digests, not the document, and
//! `weft.deployments.PlanFromSource` later reads the submitted architecture, so the control host
//! keeps the file (`ess/cloud/11-translation/deployments.yaml`, the comment of `PlanFromSource`).
//! One table of the state file, [`TABLE`], created with the others ([`crate::storage::Store::open`]):
//! a document written in a unit of work ([`crate::storage::Store::transaction`]) commits or rolls
//! back with the unit's rows. No command deletes a source, so no document is deleted.

use weft_types::deployments::SourceDigest;

use crate::Ports;

/// The table of the documents, by source digest.
pub const TABLE: &str = "weft.impl.Document";

/// Keeps `bytes` under `source_digest`, replacing what was held, in the open transaction if any.
pub fn put(ports: &Ports, source_digest: &SourceDigest, bytes: &[u8]) {
    ports.store().put(TABLE, source_digest, &bytes.to_vec());
}

/// The bytes kept under `source_digest`.
pub fn get(ports: &Ports, source_digest: &SourceDigest) -> Option<Vec<u8>> {
    ports.store().get(TABLE, source_digest)
}

/// Every document kept, in the order of their source digests.
pub fn list(ports: &Ports) -> Vec<Vec<u8>> {
    ports.store().list(TABLE)
}
