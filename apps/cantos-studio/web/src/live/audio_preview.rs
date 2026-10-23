//! Exact private rendition selection and transient browser playback. No public delivery authority.
#[cfg(any(target_arch = "wasm32", test))]
use super::ClientError;
use super::{admit_audio_size, Facts};
use crate::contract::AudioBinding;
use std::sync::Arc;

/// The acknowledged immutable subject, independent of a byte URL or producer capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subject {
    pub owner: String,
    pub binding: AudioBinding,
    pub submission: String,
    pub rendition: String,
    pub rendition_digest: String,
    pub sha256: String,
    pub byte_length: u64,
}
impl Subject {
    pub fn from_facts(facts: &Facts, actor: &str) -> Option<Self> {
        let work = facts.work.as_ref()?;
        let unit = facts.unit.as_ref()?;
        let submission = facts.submission.as_ref()?;
        let rendition = submission.rendition.as_ref()?;
        let binding = &submission.submission.binding;
        if actor.is_empty()
            || work.owner_id != actor
            || submission.submission.owner_id != actor
            || unit.work_id != work.id
            || binding.work_id != work.id
            || binding.unit_id != unit.id
            || rendition.submission_id != submission.submission.id
            || rendition.binding != *binding
            || rendition.verified_media.sha256 != submission.submission.expected_sha256
            || rendition.verified_media.byte_length != submission.submission.expected_byte_length
        {
            return None;
        }
        let subject = Self {
            owner: actor.into(),
            binding: binding.clone(),
            submission: submission.submission.id.clone(),
            rendition: rendition.id.clone(),
            rendition_digest: rendition.digest.clone(),
            sha256: rendition.verified_media.sha256.clone(),
            byte_length: rendition.verified_media.byte_length,
        };
        subject.valid().then_some(subject)
    }
    pub fn valid(&self) -> bool {
        !self.owner.is_empty()
            && !self.rendition_digest.is_empty()
            && admit_audio_size(self.byte_length).is_ok()
            && self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
    /// Enforce the cumulative stream bound before appending another chunk.
    pub fn accepts_chunk(&self, accumulated: usize, chunk: usize) -> bool {
        self.valid()
            && accumulated
                .checked_add(chunk)
                .is_some_and(|length| length as u64 <= self.byte_length)
    }
}

/// The browser observed actual SHA-256 and length before constructing this result.
/// Raw response JSON cannot manufacture it, and no credential or object URL is retained here.
#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedAudio {
    subject: Subject,
    bytes: Arc<Vec<u8>>,
}
impl std::fmt::Debug for VerifiedAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifiedAudio")
            .field("subject", &self.subject)
            .field("byte_length", &self.bytes.len())
            .field("bytes", &"[redacted]")
            .finish()
    }
}
#[cfg(any(target_arch = "wasm32", test))]
fn forgotten_preview(state: &super::RequestState) -> Option<super::RequestState> {
    if matches!(state.result(), Some(super::Reply::AudioPreview(_))) {
        state.forget_result()
    } else {
        None
    }
}
impl VerifiedAudio {
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn admit(
        subject: Subject,
        bytes: Arc<Vec<u8>>,
        observed_sha256: &str,
    ) -> Result<Self, ClientError> {
        if !subject.valid()
            || bytes.len() as u64 != subject.byte_length
            || observed_sha256 != subject.sha256
        {
            return Err(ClientError::ReplyMismatch);
        }
        Ok(Self { subject, bytes })
    }
    pub fn subject(&self) -> &Subject {
        &self.subject
    }
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
    pub fn answers(&self, subject: &Subject) -> bool {
        self.subject == *subject && self.bytes.len() as u64 == subject.byte_length
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use crate::live::{
        copy::{text, Key},
        workspace::Dispatch,
        Command, Reply,
    };
    use crate::ui::{ActionButton, BlockReason};
    use leptos::prelude::*;
    use wasm_bindgen::{JsCast, JsValue};

    #[derive(Clone)]
    struct Loaded {
        subject: Subject,
        url: String,
    }

    mod platform {
        use super::*;
        fn media_call(value: &JsValue, name: &str) -> Result<(), ClientError> {
            let function = js_sys::Reflect::get(value, &JsValue::from_str(name))
                .map_err(|_| ClientError::Transport)?
                .dyn_into::<js_sys::Function>()
                .map_err(|_| ClientError::Transport)?;
            function.call0(value).map_err(|_| ClientError::Transport)?;
            Ok(())
        }
        pub fn stop(node: NodeRef<leptos::html::Audio>) -> Result<(), ClientError> {
            let Some(element) = node.get_untracked() else {
                return Ok(());
            };
            let value: &JsValue = element.as_ref();
            let paused = media_call(value, "pause");
            element
                .remove_attribute("src")
                .map_err(|_| ClientError::Transport)?;
            let unloaded = media_call(value, "load");
            paused.and(unloaded)
        }
        pub fn revoke(url: &str) {
            let _ = web_sys::Url::revoke_object_url(url);
        }
        pub fn url(audio: &VerifiedAudio) -> Result<String, ClientError> {
            let parts = js_sys::Array::new();
            parts.push(&js_sys::Uint8Array::from(audio.bytes()));
            let options = web_sys::BlobPropertyBag::new();
            options.set_type("audio/ogg");
            let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
                .map_err(|_| ClientError::Transport)?;
            web_sys::Url::create_object_url_with_blob(&blob).map_err(|_| ClientError::Transport)
        }
    }

    #[component]
    pub(in crate::live) fn AudioPreview(
        facts: RwSignal<Facts>,
        dispatch: Dispatch,
        actor: Signal<String>,
        active: Signal<bool>,
        session_pending: Signal<bool>,
        english: Signal<bool>,
        blocked: Signal<bool>,
    ) -> impl IntoView {
        let loaded = RwSignal::new(None::<Loaded>);
        let failed = RwSignal::new(false);
        let audio = NodeRef::<leptos::html::Audio>::new();
        let subject = Memo::new(move |_| facts.with(|f| Subject::from_facts(f, &actor.get())));
        let usable = Signal::derive(move || {
            active.get() && !session_pending.get() && subject.with(Option::is_some)
        });
        let forget = move || {
            let next = dispatch.state.with_untracked(forgotten_preview);
            if let Some(next) = next {
                dispatch.state.set(next);
            }
        };
        let clear = move || {
            let _ = platform::stop(audio);
            if let Some(previous) = loaded.get_untracked() {
                platform::revoke(&previous.url);
                loaded.set(None);
            }
            failed.set(false);
            forget();
        };
        Effect::new(move |_| {
            let current = subject.get();
            if !usable.get()
                || loaded.with_untracked(|value| {
                    value
                        .as_ref()
                        .is_some_and(|value| Some(&value.subject) != current.as_ref())
                })
            {
                clear();
            }
            let reply = dispatch.state.with(|state| match state.result() {
                Some(Reply::AudioPreview(reply)) => Some(reply.clone()),
                _ => None,
            });
            if let Some(reply) = reply {
                if usable.get()
                    && current
                        .as_ref()
                        .is_some_and(|subject| reply.answers(subject))
                {
                    match platform::url(&reply) {
                        Ok(url) => {
                            let _ = platform::stop(audio);
                            if let Some(previous) = loaded.get_untracked() {
                                platform::revoke(&previous.url);
                            }
                            loaded.set(Some(Loaded {
                                subject: reply.subject().clone(),
                                url,
                            }));
                            failed.set(false);
                        }
                        Err(_) => failed.set(true),
                    }
                }
                forget();
            }
        });
        on_cleanup(move || {
            let _ = platform::stop(audio);
            if let Some(previous) = loaded.try_get_untracked().flatten() {
                platform::revoke(&previous.url);
            }
            if let Some(next) = dispatch
                .state
                .try_with_untracked(forgotten_preview)
                .flatten()
            {
                let _ = dispatch.state.try_set(next);
            }
        });
        view! {<section class="live-section" aria-labelledby="live-audio-preview-title">
            <h4 id="live-audio-preview-title">{move ||text(Key::AudioPreview,english.get())}</h4>
            <p class="help">{move ||text(Key::AudioPreviewHelp,english.get())}</p>
            <ActionButton id="live-load-audio-preview" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get() || !usable.get()) on_press=Callback::new(move |_| {
                if blocked.get_untracked() || !usable.get_untracked() { return; }
                if let Some(subject) = subject.get_untracked() { failed.set(false); dispatch.send(Command::PreviewAudio {subject}); }
            })>{move ||text(Key::LoadAudioPreview,english.get())}</ActionButton>
            <Show when=move || usable.get() && loaded.with(Option::is_some)>
                <audio node_ref=audio controls preload="none" src=move ||loaded.with(|value|value.as_ref().map(|value|value.url.clone())) aria-label=move ||text(Key::AudioPreview,english.get()) on:error=move |_|failed.set(true)/>
                <ActionButton id="live-clear-audio-preview" english=english on_press=Callback::new(move |_|clear())>{move ||text(Key::ClearAudioPreview,english.get())}</ActionButton>
            </Show>
            <p role="status">{move ||if failed.get() {text(Key::AudioPreviewFailed,english.get())} else {""}}</p>
        </section>}
    }
}
#[cfg(target_arch = "wasm32")]
pub(in crate::live) use browser::AudioPreview;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::*;
    use crate::live::{Command, Credentials, Phase, Reply, RequestState};
    const UNIT: &str = "20000000-0000-4000-8000-000000000002";
    fn subject() -> Subject {
        let rendition: AudioRenditionDocument = serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/rendition-response.json")).unwrap();
        Subject {
            owner: "owner".into(),
            binding: rendition.binding,
            submission: rendition.submission_id,
            rendition: rendition.id,
            rendition_digest: rendition.digest,
            sha256: "a".repeat(64),
            byte_length: 3,
        }
    }
    #[test]
    fn audio_preview_is_an_exact_owner_read_and_never_a_json_acknowledgement() {
        let subject = subject();
        let command = Command::PreviewAudio {
            subject: subject.clone(),
        };
        assert_eq!(
            command.path().unwrap(),
            format!(
                "/api/v1/studio/content-units/{UNIT}/audio-submissions/{}/renditions/{}/bytes",
                subject.submission, subject.rendition
            )
        );
        assert_eq!(command.method(), "GET");
        assert_eq!(command.credentials(), Credentials::OwnerSession);
        assert!(command.producer_credential().is_none());
        assert_eq!(command.json().unwrap(), None);
        assert_eq!(
            command.decode(200, b"{}"),
            Err(ClientError::Protocol { status: 200 })
        );
    }
    #[test]
    fn audio_preview_refuses_hash_length_and_binding_mismatch() {
        let expected = subject();
        let bytes = Arc::new(vec![1, 2, 3]);
        assert_eq!(
            VerifiedAudio::admit(expected.clone(), bytes.clone(), &"b".repeat(64)),
            Err(ClientError::ReplyMismatch)
        );
        assert_eq!(
            VerifiedAudio::admit(expected.clone(), Arc::new(vec![1, 2]), &expected.sha256),
            Err(ClientError::ReplyMismatch)
        );
        let verified = VerifiedAudio::admit(expected.clone(), bytes, &expected.sha256).unwrap();
        let mut wrong = expected.clone();
        wrong.binding.brief_version += 1;
        assert!(!verified.answers(&wrong));
        assert_eq!(
            Command::PreviewAudio { subject: wrong }.check_reply(&Reply::AudioPreview(verified)),
            Err(ClientError::ReplyMismatch)
        );
    }
    #[test]
    fn audio_preview_stream_rejects_extra_bytes_and_overflow_before_append() {
        let subject = subject();
        assert!(subject.accepts_chunk(1, 2));
        assert!(!subject.accepts_chunk(1, 3));
        assert!(!subject.accepts_chunk(usize::MAX, 1));
        let mut huge = subject;
        huge.byte_length = super::super::MAX_AUDIO_BYTES + 1;
        assert!(!huge.accepts_chunk(0, 1));
    }
    #[test]
    fn audio_preview_acknowledgement_cannot_reappear_after_account_switch() {
        let subject = subject();
        let reply = VerifiedAudio::admit(subject.clone(), Arc::new(vec![1, 2, 3]), &subject.sha256)
            .unwrap();
        let state = RequestState::default()
            .context("owner", "workspace")
            .unwrap();
        let (pending, ticket) = state.begin(Command::PreviewAudio { subject }).unwrap();
        let changed = pending.context("other-owner", "workspace").unwrap();
        assert_eq!(
            changed
                .finish(ticket, Ok(Reply::AudioPreview(reply)))
                .phase(),
            Phase::Idle
        );
    }
    #[test]
    fn preview_cleanup_has_no_publication_without_a_completed_preview() {
        assert!(forgotten_preview(&RequestState::default()).is_none());
        let subject = subject();
        let verified =
            VerifiedAudio::admit(subject.clone(), Arc::new(vec![1, 2, 3]), &subject.sha256)
                .unwrap();
        let debug = format!("{verified:?}");
        assert!(debug.contains("[redacted]"));
        assert!(!debug.contains("[1, 2, 3]"));
        let state = RequestState::default()
            .context("owner", "workspace")
            .unwrap();
        let (pending, ticket) = state.begin(Command::PreviewAudio { subject }).unwrap();
        assert!(forgotten_preview(&pending).is_none());
        let ready = pending.finish(ticket, Ok(Reply::AudioPreview(verified)));
        let cleared = forgotten_preview(&ready).unwrap();
        assert_eq!(cleared.phase(), Phase::Idle);
        assert!(forgotten_preview(&cleared).is_none());
    }
}
