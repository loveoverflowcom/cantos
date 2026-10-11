//! TTS adapters and offline receipt inspection. No dispatcher, store or authorization is created.
pub mod attempt;
pub mod audio;
pub mod google;
pub mod google_contract;
pub mod google_http;
pub mod local;
pub mod local_contract;
mod receipt;

pub use receipt::{
    audit_receipt, decode_document, decode_receipt, ChargeObservation, ReceiptAudit, ReceiptError,
    ReceiptEvidence, ReceiptOutcome, TtsProviderIdentity, TtsReceipt, UsageObservation,
    MAX_RECEIPT_BYTES, RECEIPT_VERSION,
};
