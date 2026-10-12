//! Provider identities and declared capabilities that casting may bind. These are data, not
//! adapters: no process, SDK, network or file access lives here. Synthesis adapters conform to
//! the contracts recorded here; casting never asks an adapter what it supports.
use cantos_contracts::{ProductionCapability, ProductionCatalogResponse};

pub mod google;
pub mod local;
pub mod neural;

pub const CATALOG_VERSION: &str = "production-reference-v1";
/// Identity of the reference-only planning entries, which no adapter executes.
pub const ADAPTER_VERSION: &str = "reference-only-no-dispatch-v1";
pub const OUTPUT_CONTRACT: &str = "reference-only/no-audio";
pub(super) const LANGUAGES: [&str; 4] = ["vi", "vi-VN", "en", "en-US"];
const EMOTIONS: [&str; 6] = ["neutral", "calm", "hopeful", "warm", "sad", "angry"];

/// Explicit planning capabilities, not provider availability or real voice claims.
pub fn production_catalog() -> ProductionCatalogResponse {
    let first = ProductionCapability {
        provider_id: "reference-only".into(),
        model_id: "reference-v1".into(),
        voice_ids: vec!["synthetic-narrator".into(), "synthetic-character".into()],
        languages: LANGUAGES.iter().map(|s| (*s).into()).collect(),
        emotions: EMOTIONS.iter().map(|s| (*s).into()).collect(),
        rate_min_permille: 500,
        rate_max_permille: 2_000,
        pitch_min_semitones: -12,
        pitch_max_semitones: 12,
        intensity_supported: true,
        pronunciation_supported: true,
        reference_only: true,
        billable_dispatch_available: false,
    };
    let mut second = first.clone();
    second.model_id = "reference-v2".into();
    let mut capabilities = vec![first, second, local::capability(), neural::capability()];
    capabilities.extend(google::capabilities());
    ProductionCatalogResponse {
        version: CATALOG_VERSION.into(),
        capabilities,
    }
}

/// What casting checks before it freezes a line for an executable adapter, and the adapter
/// identity the frozen line then carries.
///
/// Every executable adapter supports only the explicit `neutral` delivery at intensity 0. For
/// Google that is the voice's default delivery: nothing is mapped implicitly.
#[derive(Clone, Copy)]
pub struct CastingContract {
    pub adapter_version: &'static str,
    pub output_contract: &'static str,
    /// Admission of the exact text sent, after pronunciation replacements.
    pub valid_text: fn(&str) -> bool,
    pub max_text_bytes: usize,
    /// Rule codes for text over `max_text_bytes` and for any other inadmissible text.
    pub text_too_large: &'static str,
    pub invalid_text: &'static str,
}

/// The executable adapter contract for `provider_id`; `None` for reference-only entries.
pub fn casting_contract(provider_id: &str) -> Option<CastingContract> {
    match provider_id {
        local::PROVIDER_ID => Some(CastingContract {
            adapter_version: local::ADAPTER_VERSION,
            output_contract: local::OUTPUT_CONTRACT,
            valid_text: local::valid_text,
            max_text_bytes: local::MAX_TEXT_BYTES,
            text_too_large: "local_text_too_large",
            invalid_text: "invalid_local_text",
        }),
        google::PROVIDER_ID => Some(CastingContract {
            adapter_version: google::ADAPTER_VERSION,
            output_contract: google::OUTPUT_CONTRACT,
            valid_text: google::valid_text,
            max_text_bytes: google::MAX_TEXT_BYTES,
            text_too_large: "google_text_too_large",
            invalid_text: "invalid_google_text",
        }),
        neural::PROVIDER_ID => Some(CastingContract {
            adapter_version: neural::ADAPTER_VERSION,
            output_contract: neural::OUTPUT_CONTRACT,
            valid_text: neural::valid_text,
            max_text_bytes: neural::MAX_TEXT_BYTES,
            text_too_large: "neural_text_too_large",
            invalid_text: "invalid_neural_text",
        }),
        _ => None,
    }
}

impl CastingContract {
    /// The rule code for inadmissible `text`, or `None` if the adapter accepts it.
    pub fn text_issue(&self, text: &str) -> Option<&'static str> {
        if (self.valid_text)(text) {
            None
        } else if text.len() > self.max_text_bytes {
            Some(self.text_too_large)
        } else {
            Some(self.invalid_text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_entries_have_no_executable_contract() {
        let catalog = production_catalog();
        for capability in &catalog.capabilities {
            assert_eq!(
                casting_contract(&capability.provider_id).is_some(),
                !capability.reference_only,
                "{}",
                capability.provider_id
            );
        }
        assert!(casting_contract("unknown-provider").is_none());
    }

    #[test]
    fn each_contract_carries_its_adapter_identity_and_rule_codes() {
        for (provider, adapter, output, max, prefix) in [
            (
                local::PROVIDER_ID,
                local::ADAPTER_VERSION,
                local::OUTPUT_CONTRACT,
                local::MAX_TEXT_BYTES,
                "local",
            ),
            (
                google::PROVIDER_ID,
                google::ADAPTER_VERSION,
                google::OUTPUT_CONTRACT,
                google::MAX_TEXT_BYTES,
                "google",
            ),
            (
                neural::PROVIDER_ID,
                neural::ADAPTER_VERSION,
                neural::OUTPUT_CONTRACT,
                neural::MAX_TEXT_BYTES,
                "neural",
            ),
        ] {
            let contract = casting_contract(provider).unwrap();
            assert_eq!(contract.adapter_version, adapter);
            assert_eq!(contract.output_contract, output);
            assert_eq!(contract.max_text_bytes, max);
            assert_eq!(contract.text_issue("Xin chào."), None, "{provider}");
            assert_eq!(
                contract.text_issue(" "),
                Some(format!("invalid_{prefix}_text").as_str())
            );
            assert_eq!(
                contract.text_issue(&"a".repeat(max + 1)),
                Some(format!("{prefix}_text_too_large").as_str())
            );
        }
    }

    #[test]
    fn each_contract_admits_text_with_its_own_adapter_rule() {
        // eSpeak reads `[[` as phoneme input; Google and the neural helper refuse controls.
        let local = casting_contract(local::PROVIDER_ID).unwrap();
        let google = casting_contract(google::PROVIDER_ID).unwrap();
        let neural = casting_contract(neural::PROVIDER_ID).unwrap();
        assert_eq!(local.text_issue("a [[b"), Some("invalid_local_text"));
        assert_eq!(google.text_issue("a [[b"), None);
        assert_eq!(neural.text_issue("a [[b"), Some("invalid_neural_text"));
        assert_eq!(google.text_issue("a\u{2028}b"), Some("invalid_google_text"));
    }
}
