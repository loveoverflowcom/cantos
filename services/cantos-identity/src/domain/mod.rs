//! Checked facts are shape-valid, not evidence of possession or caller authority.

pub mod admission;
pub mod email;
pub mod ids;
pub mod mailbox_proof;
#[cfg(feature = "service")]
pub mod password;
pub mod security_version;
