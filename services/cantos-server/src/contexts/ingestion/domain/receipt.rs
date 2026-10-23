//! Rules for an import receipt: admissible metadata, the stored text projection and the digest
//! that binds the original bytes, metadata and extraction outcome.
use crate::contract::studio::{FieldIssue, ImportMetadata, ImportOutcome, ImportRequest};
use sha2::{Digest, Sha256};

use crate::platform::ids::canonical_uuid;

use super::extraction::MAX_SOURCE_BYTES;

/// Intrinsic metadata/size admission only. This is not an ownership, extraction,
/// rights, receipt-integrity or current-authorization witness. Original bytes are immutable.
/// No Deserialize, Default, public fields or unchecked conversion enters this boundary.
#[derive(Debug)]
pub(crate) struct AdmittedImport {
    metadata: ImportMetadata,
    original_bytes: Vec<u8>,
}

impl TryFrom<ImportRequest> for AdmittedImport {
    type Error = Vec<FieldIssue>;

    fn try_from(raw: ImportRequest) -> Result<Self, Self::Error> {
        let mut issues = metadata_issues(&raw.metadata);
        if raw.original_bytes.len() > MAX_SOURCE_BYTES {
            issues.push(field_issue("/original_bytes", "byte_length"));
        }
        if !issues.is_empty() {
            return Err(issues);
        }
        Ok(Self {
            metadata: raw.metadata,
            original_bytes: raw.original_bytes,
        })
    }
}

impl AdmittedImport {
    pub(crate) fn metadata(&self) -> &ImportMetadata {
        &self.metadata
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.original_bytes
    }

    /// Dropping admission when projecting back to a raw wire response is deliberate.
    pub(crate) fn into_raw(self) -> ImportRequest {
        ImportRequest {
            metadata: self.metadata,
            original_bytes: self.original_bytes,
        }
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Binds the exact bytes, the supplied metadata and the extraction outcome of one import.
pub fn receipt_digest(
    metadata: &ImportMetadata,
    outcome: &ImportOutcome,
    sha256: &str,
) -> Result<String, serde_json::Error> {
    // Hash typed serialization, never JSONB's object ordering or PostgreSQL pretty printing.
    let metadata = serde_json::to_vec(metadata)?;
    let outcome = serde_json::to_vec(outcome)?;
    let mut digest = Sha256::new();
    digest.update(b"cantos/manuscript-import/r1\n");
    digest.update(sha256.as_bytes());
    digest.update(b"\n");
    digest.update(metadata);
    digest.update(b"\n");
    digest.update(outcome);
    Ok(format!("{:x}", digest.finalize()))
}

pub fn field_issue(path: &str, rule: &str) -> FieldIssue {
    FieldIssue {
        path: path.into(),
        rule: rule.into(),
    }
}

/// Every independent problem with caller-supplied metadata, with safe paths and rule codes.
pub fn metadata_issues(metadata: &ImportMetadata) -> Vec<FieldIssue> {
    fn display_issues(value: &str, limit: usize, path: &str, issues: &mut Vec<FieldIssue>) {
        if value.trim().is_empty() {
            issues.push(field_issue(path, "nonblank"));
        }
        if value.len() > limit {
            issues.push(field_issue(path, "byte_length"));
        }
        if value.chars().any(char::is_control) {
            issues.push(field_issue(path, "control_character"));
        }
    }
    let mut issues = Vec::new();
    if canonical_uuid(&metadata.operation_id).is_none() {
        issues.push(field_issue("/metadata/operation_id", "canonical_uuid"));
    }
    display_issues(&metadata.file_name, 255, "/metadata/file_name", &mut issues);
    display_issues(
        &metadata.reference,
        2048,
        "/metadata/reference",
        &mut issues,
    );
    for (value, path) in [
        (&metadata.rights_holder, "/metadata/rights_holder"),
        (
            &metadata.permission_evidence,
            "/metadata/permission_evidence",
        ),
        (&metadata.usage_scope, "/metadata/usage_scope"),
    ] {
        if let Some(value) = value {
            display_issues(value, 2048, path, &mut issues);
        }
    }
    issues
}

pub fn source_id_valid(id: &str) -> bool {
    id.len() <= 64
        && !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
}

/// The text projection stored beside the original bytes, when one exists.
pub fn original_text(bytes: &[u8]) -> Option<String> {
    // PostgreSQL text cannot contain NUL; the original bytes remain the authority.
    std::str::from_utf8(bytes)
        .ok()
        .filter(|text| !text.is_empty() && !text.contains('\0'))
        .map(str::to_owned)
}

/// A legacy text-only source: its stored text and optional bytes must agree with its checksum.
pub fn legacy_source_intact(original_text: &str, bytes: Option<&[u8]>, sha256: &str) -> bool {
    bytes.is_none_or(|bytes| bytes == original_text.as_bytes())
        && sha256_hex(bytes.unwrap_or(original_text.as_bytes())) == sha256
}

#[cfg(test)]
mod tests {
    use crate::contract::studio::{ImportFormat, ImportMetadata};

    use super::{field_issue, legacy_source_intact, metadata_issues, sha256_hex};

    #[test]
    fn a_legacy_source_must_agree_with_its_checksum() {
        let text = "Lời người dẫn chuyện.";
        let checksum = sha256_hex(text.as_bytes());
        assert!(legacy_source_intact(text, None, &checksum));
        assert!(legacy_source_intact(text, Some(text.as_bytes()), &checksum));
        assert!(!legacy_source_intact(
            text,
            Some("Lời khác.".as_bytes()),
            &checksum
        ));
        assert!(!legacy_source_intact(text, None, &sha256_hex(b"other")));
    }

    #[test]
    fn metadata_diagnostics_collect_independent_safe_paths_and_rules() {
        let metadata = ImportMetadata {
            operation_id: "invalid operation".into(),
            file_name: "Đ".repeat(128),
            format: ImportFormat::Txt,
            reference: " \n".into(),
            rights_holder: Some(" ".into()),
            permission_evidence: None,
            usage_scope: Some("private\0scope".into()),
        };
        assert_eq!(
            metadata_issues(&metadata),
            vec![
                field_issue("/metadata/operation_id", "canonical_uuid"),
                field_issue("/metadata/file_name", "byte_length"),
                field_issue("/metadata/reference", "nonblank"),
                field_issue("/metadata/reference", "control_character"),
                field_issue("/metadata/rights_holder", "nonblank"),
                field_issue("/metadata/usage_scope", "control_character"),
            ]
        );
    }
}

#[cfg(test)]
mod admission_tests {
    use proptest::prelude::*;

    use super::*;
    use crate::contract::studio::ImportFormat;

    fn request(bytes: Vec<u8>) -> ImportRequest {
        ImportRequest {
            metadata: ImportMetadata {
                operation_id: "00000000-0000-4000-8000-000000000001".into(),
                file_name: "Nguồn.txt".into(),
                format: ImportFormat::Txt,
                reference: "Synthetic admission fixture".into(),
                rights_holder: None,
                permission_evidence: None,
                usage_scope: None,
            },
            original_bytes: bytes,
        }
    }

    #[test]
    fn admission_accepts_the_byte_limit_and_preserves_failed_extraction_inputs() {
        // Empty/malformed bytes still produce an immutable failed extraction receipt.
        // Admitting intrinsic size/metadata must not invent parser success.
        for bytes in [
            vec![],
            vec![0xff, 0, b'\r', b'\n'],
            vec![b'a'; MAX_SOURCE_BYTES],
        ] {
            let raw = request(bytes);
            let admitted = AdmittedImport::try_from(raw.clone()).unwrap();
            assert_eq!(admitted.bytes(), raw.original_bytes);
            assert_eq!(admitted.metadata(), &raw.metadata);
            assert_eq!(admitted.into_raw(), raw);
        }
        assert_eq!(
            AdmittedImport::try_from(request(vec![0; MAX_SOURCE_BYTES + 1])).unwrap_err(),
            vec![field_issue("/original_bytes", "byte_length")]
        );
    }

    #[test]
    fn admission_collects_metadata_and_size_errors_in_the_existing_order() {
        let mut raw = request(vec![0; MAX_SOURCE_BYTES + 1]);
        raw.metadata.operation_id = "bad".into();
        raw.metadata.file_name = " ".into();
        assert_eq!(
            AdmittedImport::try_from(raw).unwrap_err(),
            vec![
                field_issue("/metadata/operation_id", "canonical_uuid"),
                field_issue("/metadata/file_name", "nonblank"),
                field_issue("/original_bytes", "byte_length"),
            ]
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 128, rng_seed: proptest::test_runner::RngSeed::Fixed(20_261_016), ..ProptestConfig::default() })]
        #[test]
        fn admitted_originals_remain_byte_exact(bytes in proptest::collection::vec(any::<u8>(), 0..257)) {
            let raw = request(bytes);
            let admitted = AdmittedImport::try_from(raw.clone()).unwrap();
            prop_assert_eq!(admitted.bytes(), raw.original_bytes.as_slice());
            prop_assert_eq!(admitted.into_raw(), raw);
        }
    }
}
