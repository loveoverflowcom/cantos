//! Finite, versioned stdio protocol for the packaged-worker feasibility boundary.
//!
//! No request can choose a workspace, process, provider, credential or generation operation.
//! The wrapper authenticates before invoking this process; stdio is not an auth boundary.

use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
};

use serde::{Deserialize, Serialize};

use crate::{
    domain::checks::check_text,
    wire::{detail_to_wire, DetailWire, OutcomeWire},
};

pub const SCHEMA_VERSION: &str = "cantos-forge.worker/v1";
pub const MAX_REQUEST_BYTES: usize = 32_768;
pub const MAX_TEXT_BYTES: usize = 16_384;
pub const MAX_REPLY_BYTES: usize = 8_192;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema_version: String,
    request_id: String,
    operation: Operation,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    // A struct variant is deliberate: serde ignores extra fields on a unit variant even
    // with deny_unknown_fields, which would weaken this finite protocol.
    Capabilities {},
    CheckText { text: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub schema_version: &'static str,
    pub request_id: Option<String>,
    pub result: WorkerResult,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerResult {
    Capabilities {
        operations: [&'static str; 2],
        generation: &'static str,
    },
    TextChecked {
        check: String,
        outcome: OutcomeWire,
        detail: BTreeMap<String, DetailWire>,
    },
    Rejected {
        code: Refusal,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    RequestTooLarge,
    InvalidRequest,
    UnsupportedVersion,
    InvalidRequestId,
    TextTooLarge,
    RequestReadFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Completed,
    Rejected,
}

impl Status {
    pub fn code(self) -> u8 {
        match self {
            Self::Completed => 0,
            Self::Rejected => 2,
        }
    }
}

impl Reply {
    fn rejected(request_id: Option<String>, code: Refusal) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            request_id,
            result: WorkerResult::Rejected { code },
        }
    }

    pub fn status(&self) -> Status {
        match self.result {
            WorkerResult::Rejected { .. } => Status::Rejected,
            WorkerResult::Capabilities { .. } | WorkerResult::TextChecked { .. } => {
                Status::Completed
            }
        }
    }
}

/// All business checks delegate to the existing pure core. The process never persists content.
pub fn handle(bytes: &[u8]) -> Reply {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Reply::rejected(None, Refusal::RequestTooLarge);
    }
    let request: Request = match serde_json::from_slice(bytes) {
        Ok(request) => request,
        Err(_) => return Reply::rejected(None, Refusal::InvalidRequest),
    };
    if !valid_request_id(&request.request_id) {
        return Reply::rejected(None, Refusal::InvalidRequestId);
    }
    let request_id = Some(request.request_id);
    if request.schema_version != SCHEMA_VERSION {
        return Reply::rejected(request_id, Refusal::UnsupportedVersion);
    }
    let result = match request.operation {
        Operation::Capabilities {} => WorkerResult::Capabilities {
            operations: ["capabilities", "check_text"],
            generation: "unavailable",
        },
        Operation::CheckText { text } => {
            if text.len() > MAX_TEXT_BYTES {
                return Reply::rejected(request_id, Refusal::TextTooLarge);
            }
            let checked = check_text(text.as_bytes());
            WorkerResult::TextChecked {
                check: checked.check.label(),
                outcome: checked.outcome.into(),
                detail: detail_to_wire(&checked.detail),
            }
        }
    };
    Reply {
        schema_version: SCHEMA_VERSION,
        request_id,
        result,
    }
}

fn valid_request_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Read at most limit + 1, emit one bounded JSON reply, then leave process completion to main.
pub fn serve(reader: impl Read, mut writer: impl Write) -> io::Result<Status> {
    let mut bytes = Vec::new();
    let reply = match reader
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
    {
        Ok(_) => handle(&bytes),
        Err(_) => Reply::rejected(None, Refusal::RequestReadFailed),
    };
    let serialized = serde_json::to_vec(&reply).map_err(io::Error::other)?;
    if serialized.len() + 1 > MAX_REPLY_BYTES {
        return Err(io::Error::other("worker reply exceeds protocol limit"));
    }
    writer.write_all(&serialized)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(reply.status())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"schema_version":SCHEMA_VERSION,"request_id":"test-01","operation":operation})).unwrap()
    }

    #[test]
    fn capabilities_never_claim_generation_is_available() {
        assert_eq!(
            handle(&request(serde_json::json!({"kind":"capabilities"}))).result,
            WorkerResult::Capabilities {
                operations: ["capabilities", "check_text"],
                generation: "unavailable"
            }
        );
    }

    #[test]
    fn text_checks_reuse_exact_shared_domain_measurements_without_echoing_text() {
        let text = "Lời hẹn cuối mùa mưa.\n";
        let reply = handle(&request(
            serde_json::json!({"kind":"check_text","text":text}),
        ));
        let expected = check_text(text.as_bytes());
        assert_eq!(
            reply.result,
            WorkerResult::TextChecked {
                check: "utf8-text-v1".to_owned(),
                outcome: OutcomeWire::Passed,
                detail: detail_to_wire(&expected.detail)
            }
        );
        assert!(!String::from_utf8(serde_json::to_vec(&reply).unwrap())
            .unwrap()
            .contains(text));
    }

    #[test]
    fn a_failed_text_check_is_a_completed_measurement_not_a_protocol_rejection() {
        let reply = handle(&request(
            serde_json::json!({"kind":"check_text","text":" \n"}),
        ));
        assert_eq!(reply.status(), Status::Completed);
        assert!(matches!(
            reply.result,
            WorkerResult::TextChecked {
                outcome: OutcomeWire::Failed,
                ..
            }
        ));
    }

    #[test]
    fn process_configuration_and_unknown_fields_are_rejected_atomically() {
        for field in [
            "executable",
            "argv",
            "env",
            "root",
            "workspace",
            "credential",
            "unexpected",
        ] {
            let mut value: serde_json::Value =
                serde_json::from_slice(&request(serde_json::json!({"kind":"capabilities"})))
                    .unwrap();
            value[field] = serde_json::json!("untrusted");
            assert_eq!(
                handle(&serde_json::to_vec(&value).unwrap()),
                Reply::rejected(None, Refusal::InvalidRequest),
                "{field}"
            );
            let operation = serde_json::json!({"kind":"capabilities",(field):"untrusted"});
            assert_eq!(
                handle(&request(operation)),
                Reply::rejected(None, Refusal::InvalidRequest),
                "nested {field}"
            );
        }
        assert_eq!(
            handle(&request(serde_json::json!({"kind":"generate"}))),
            Reply::rejected(None, Refusal::InvalidRequest)
        );
    }

    #[test]
    fn malformed_duplicate_trailing_and_unknown_version_inputs_are_refused() {
        for bytes in [b"".as_slice(), b"{}", b"null", b"{", br#"{"schema_version":"cantos-forge.worker/v1","request_id":"a","request_id":"b","operation":{"kind":"capabilities"}}"#, br#"{"schema_version":"cantos-forge.worker/v1","request_id":"a","operation":{"kind":"capabilities"}} {}"#] {
            assert_eq!(handle(bytes), Reply::rejected(None, Refusal::InvalidRequest));
        }
        let bytes = br#"{"schema_version":"cantos-forge.worker/v2","request_id":"a","operation":{"kind":"capabilities"}}"#;
        assert_eq!(
            handle(bytes),
            Reply::rejected(Some("a".to_owned()), Refusal::UnsupportedVersion)
        );
    }

    #[test]
    fn request_ids_cannot_smuggle_text_or_control_characters_into_results() {
        for id in ["", "a b", "Nguyễn", "a\n", &"a".repeat(65)] {
            let bytes = serde_json::to_vec(&serde_json::json!({"schema_version":SCHEMA_VERSION,"request_id":id,"operation":{"kind":"capabilities"}})).unwrap();
            assert_eq!(
                handle(&bytes),
                Reply::rejected(None, Refusal::InvalidRequestId)
            );
        }
    }

    #[test]
    fn byte_limits_are_inclusive_and_bounded_reader_stops_at_one_extra_byte() {
        assert_eq!(
            handle(&request(
                serde_json::json!({"kind":"check_text","text":"x".repeat(MAX_TEXT_BYTES + 1)})
            ))
            .result,
            WorkerResult::Rejected {
                code: Refusal::TextTooLarge
            }
        );
        assert_eq!(
            handle(&request(
                serde_json::json!({"kind":"check_text","text":"x".repeat(MAX_TEXT_BYTES)})
            ))
            .status(),
            Status::Completed
        );
        let mut padded = request(serde_json::json!({"kind":"capabilities"}));
        padded.resize(MAX_REQUEST_BYTES, b' ');
        assert_eq!(handle(&padded).status(), Status::Completed);
        padded.push(b' ');
        assert_eq!(
            handle(&padded),
            Reply::rejected(None, Refusal::RequestTooLarge)
        );
        let mut infinite = io::repeat(b' ');
        let mut out = Vec::new();
        assert_eq!(serve(&mut infinite, &mut out).unwrap(), Status::Rejected);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&out).unwrap()["result"]["code"],
            "request_too_large"
        );
        assert!(out.len() < MAX_REPLY_BYTES);
    }

    #[test]
    fn read_and_write_failures_do_not_claim_a_completed_response() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("private path"))
            }
        }
        let mut output = Vec::new();
        assert_eq!(serve(Broken, &mut output).unwrap(), Status::Rejected);
        assert!(!String::from_utf8(output).unwrap().contains("private path"));
        assert_eq!(serve(io::empty(), io::sink()).unwrap(), Status::Rejected);
        assert_eq!(
            serve(io::empty(), io::Cursor::new(&mut [0_u8; 1][..]))
                .unwrap_err()
                .kind(),
            io::ErrorKind::WriteZero
        );
    }
}
