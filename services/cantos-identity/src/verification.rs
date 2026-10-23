//! Operation-bound verification action/custody port. This is distinct from generic mail
//! delivery: only a consumed external action releases the matching native proof internally.
use crate::{
    domain::{ids::OperationId, mailbox_proof::ProofId},
    persistence::{ProofCommandId, ProofSecret},
};
use std::{future::Future, pin::Pin};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationFault {
    Rejected,
    Uncertain,
    Configuration,
}
impl std::fmt::Display for VerificationFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Rejected => "verification request rejected",
            Self::Uncertain => "verification outcome uncertain",
            Self::Configuration => "verification configuration invalid",
        })
    }
}
impl std::error::Error for VerificationFault {}
pub type VerificationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), VerificationFault>> + Send + 'a>>;

pub trait VerificationActions: Send + Sync + std::fmt::Debug {
    fn project(&self) -> &str;
    /// Runtime-keyed exact-request binding. Never persist a fast unkeyed password digest.
    fn confirmation_tag(
        &self,
        operation: OperationId,
        proof: ProofId,
        command: ProofCommandId,
        code: &str,
        password: &str,
    ) -> Result<[u8; 32], VerificationFault>;
    fn seal(
        &self,
        operation: OperationId,
        proof: ProofId,
        email: &str,
        epoch: &[u8; 16],
        expires: u64,
        secret: &ProofSecret,
    ) -> Result<Vec<u8>, VerificationFault>;
    fn open(
        &self,
        operation: OperationId,
        proof: ProofId,
        email: &str,
        epoch: &[u8; 16],
        expires: u64,
        envelope: &[u8],
    ) -> Result<Zeroizing<[u8; 32]>, VerificationFault>;
    fn create<'a>(&'a self, operation: OperationId, email: &'a str) -> VerificationFuture<'a>;
    fn send<'a>(
        &'a self,
        operation: OperationId,
        proof: ProofId,
        email: &'a str,
    ) -> VerificationFuture<'a>;
    fn inspect<'a>(&'a self, code: &'a str, email: &'a str) -> VerificationFuture<'a>;
    fn consume<'a>(
        &'a self,
        operation: OperationId,
        code: &'a str,
        email: &'a str,
    ) -> VerificationFuture<'a>;
}

pub fn valid_action_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}
