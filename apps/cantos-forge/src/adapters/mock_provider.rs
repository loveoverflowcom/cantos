//! A deterministic mock speech provider: the default for local runs and emulators.
//!
//! It makes no network call and spends nothing. Without a script it answers every request with
//! bytes derived from the request's fingerprint, so one request always yields one result and a
//! different request yields a different one. A script makes it answer in a chosen order (a
//! rejection, an unknown outcome, specific audio), which is how failure handling is exercised
//! without a real provider.
//!
//! Its bytes are **not** audio: they are not PCM, not Ogg Opus and not a Cantos rendition. A
//! mock success is never provider-live evidence, and mock success cannot close a live check.

use std::{cell::RefCell, collections::VecDeque};

use crate::{
    application::ports::{Provider, ProviderResult, Reserved},
    domain::{attempt::AttemptNumber, generation::GenerationRequest},
};

#[derive(Debug, Default)]
pub struct MockProvider {
    script: RefCell<VecDeque<ProviderResult>>,
    calls: RefCell<Vec<AttemptNumber>>,
}

impl MockProvider {
    /// Answers every call with deterministic bytes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Answers the first calls with `results` in order, then falls back to deterministic bytes.
    pub fn scripted(results: impl IntoIterator<Item = ProviderResult>) -> Self {
        Self {
            script: RefCell::new(results.into_iter().collect()),
            calls: RefCell::default(),
        }
    }

    /// The attempt number of every call, in the order the calls were made.
    pub fn calls(&self) -> Vec<AttemptNumber> {
        self.calls.borrow().clone()
    }

    /// The bytes the unscripted mock returns for `request`.
    pub fn audio_for(request: &GenerationRequest) -> Vec<u8> {
        format!("cantos-forge.mock-audio/v1:{}", request.fingerprint()).into_bytes()
    }
}

impl Provider for MockProvider {
    fn synthesize(&self, reserved: &Reserved, request: &GenerationRequest) -> ProviderResult {
        self.calls.borrow_mut().push(reserved.attempt());
        self.script
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| ProviderResult::Audio(Self::audio_for(request)))
    }
}
