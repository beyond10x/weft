//! The decider of `external:` branches, a table in code. A branch the table does not list is not
//! taken: its command answers its default branch.
//!
//! | command                         | branch          | taken when                                          |
//! |---------------------------------|-----------------|-----------------------------------------------------|
//! | `weft.infrastructure.JoinNode`  | `token-invalid` | the token is not the one weft issued for the node   |

use sha2::{Digest, Sha256};
use weft_types::behaviour::ExternalCommand;

use crate::storage::Store;

/// Whether the external branch `outcome` of `command` is taken.
pub fn decide(store: &Store, command: ExternalCommand<'_>, outcome: &str) -> bool {
    match (command, outcome) {
        (ExternalCommand::WeftInfrastructureJoinNode(input), "token-invalid") => {
            store.join_token_digest(&input.node_id).as_deref()
                != Some(join_token_digest(&input.join_token).as_str())
        }
        _ => false,
    }
}

/// The SHA-256 of a join token, in lower-case hex: all the state file keeps of it.
pub fn join_token_digest(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
