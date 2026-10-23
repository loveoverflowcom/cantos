//! Email/password identity v1 (#335). JSON-only; the Server never returns a credential secret
//! to a browser (Web receives an HttpOnly cookie) and never accepts one in a body or URL.
use serde::{Deserialize, Serialize};
use std::fmt;

/// Starts a Theatre signup for a mailbox. The reply is generic whether or not mail was sent.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignupStart {
    pub email: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SignupAccepted {
    /// Always true for a well-formed request; it never says whether an address is registered.
    pub accepted: bool,
    /// Fixed policy lifetime of the mailed proof.
    pub proof_ttl_seconds: u32,
}

/// The mailbox owner's explicit completion. The three hex fields come from the mailed link's
/// fragment (`#operation=…&proof=…&secret=…`), which no server ever receives from a GET.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignupComplete {
    /// 32 lowercase hex characters (16 bytes).
    pub operation_id: String,
    /// 32 lowercase hex characters (16 bytes).
    pub proof_id: String,
    /// 64 lowercase hex characters (32 bytes).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub proof_secret: String,
    /// Alternative exact Firebase action; never an ID/login token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firebase_oob_code: Option<String>,
    /// Chosen by the mailbox owner; never trimmed or normalized.
    pub password: String,
    /// Must be true.
    pub explicit_confirmation: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SignupStatus {
    /// This request created the account.
    Completed,
    /// An identical earlier request created it. History, never a session.
    Replayed,
}

/// Completion creates an account and **no** session: log in with the chosen password.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SignupCompleted {
    pub status: SignupStatus,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    pub email: String,
    pub password: String,
}

impl fmt::Debug for SignupStart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignupStart([redacted])")
    }
}
impl fmt::Debug for SignupComplete {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignupComplete([redacted])")
    }
}
impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Login([redacted])")
    }
}
