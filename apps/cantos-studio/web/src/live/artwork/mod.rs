//! Studio-owned private artwork presentation. Server admission and ownership remain authoritative.
pub mod copy;
pub mod form;
#[cfg(target_arch = "wasm32")]
pub mod panel;
#[cfg(target_arch = "wasm32")]
pub mod platform;
#[cfg(all(target_arch = "wasm32", debug_assertions))]
pub mod preview;
#[cfg(test)]
mod tests;
use crate::contract::*;
use std::{collections::BTreeSet, sync::Arc};

pub const MAX_BYTES: u64 = 8 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    File,
    Dimensions,
    Provenance,
    Rights,
    Alt,
    Decode,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    pub provenance: String,
    pub rights: String,
    pub alt: String,
}
impl Draft {
    /// Local image/field edits remain unsaved until the exact selection is acknowledged.
    pub fn is_dirty(&self, has_local: bool) -> bool {
        has_local || *self != Self::default()
    }
    pub fn acknowledge_selection(&mut self, submitted: &Self, same_file: bool) -> bool {
        if same_file && self == submitted {
            *self = Self::default();
            true
        } else {
            false
        }
    }
    pub fn declarations(&self) -> Result<ImageDeclarations, Problem> {
        if self.provenance.trim().is_empty()
            || self.provenance.len() > 4096
            || self
                .provenance
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        {
            return Err(Problem::Provenance);
        }
        let refs: Vec<_> = self
            .rights
            .split(',')
            .map(str::trim)
            .map(str::to_owned)
            .collect();
        let unique: BTreeSet<_> = refs.iter().collect();
        if refs.is_empty()
            || refs.len() > 64
            || unique.len() != refs.len()
            || refs.iter().any(|r| {
                r.is_empty()
                    || r.len() > 64
                    || !r
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
            })
        {
            return Err(Problem::Rights);
        }
        let mut refs = refs;
        refs.sort();
        Ok(ImageDeclarations {
            provenance: self.provenance.clone(),
            rights_refs: refs,
        })
    }
    pub fn alt(&self) -> Result<String, Problem> {
        if self.alt.trim().is_empty()
            || self.alt.len() > 1024
            || self
                .alt
                .chars()
                .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
        {
            Err(Problem::Alt)
        } else {
            Ok(self.alt.clone())
        }
    }
}
pub fn file_bounds(mime: &str, size: u64) -> Result<ImageMime, Problem> {
    if !(1..=MAX_BYTES).contains(&size) {
        return Err(Problem::File);
    }
    match mime {
        "image/avif" => Ok(ImageMime::Avif),
        _ => Err(Problem::File),
    }
}
pub fn dimensions(width: u32, height: u32) -> Result<(), Problem> {
    if !(1..=4096).contains(&width)
        || !(1..=4096).contains(&height)
        || u64::from(width) * u64::from(height) > 16_777_216
    {
        Err(Problem::Dimensions)
    } else {
        Ok(())
    }
}
/// Identify still-AVIF before browser preview; full decode/admission remains with the Server.
pub fn avif_bytes(bytes: &[u8]) -> Result<(), Problem> {
    if bytes.len() < 16 || bytes.len() as u64 > MAX_BYTES || &bytes[4..8] != b"ftyp" {
        return Err(Problem::File);
    }
    let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if length < 16 || length > bytes.len() || !length.is_multiple_of(4) {
        return Err(Problem::File);
    }
    let compatible = bytes[16..length].as_chunks::<4>().0;
    let brands =
        std::iter::once(&bytes[8..12]).chain(compatible.iter().map(|brand| brand.as_slice()));
    let mut still = false;
    for brand in brands {
        if brand == b"avis" {
            return Err(Problem::File);
        }
        still |= brand == b"avif";
    }
    if still {
        Ok(())
    } else {
        Err(Problem::File)
    }
}
pub fn mime_name(mime: ImageMime) -> &'static str {
    match mime {
        ImageMime::Png => "image/png",
        ImageMime::Jpeg => "image/jpeg",
        ImageMime::Avif => "image/avif",
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalImage {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub sha256: String,
    pub mime: ImageMime,
    pub width: u32,
    pub height: u32,
    pub url: String,
}
impl LocalImage {
    pub fn prepare(
        &self,
        draft: &Draft,
        operation_id: String,
        id: String,
    ) -> Result<CreateImageIntentRequest, Problem> {
        Ok(CreateImageIntentRequest {
            operation_id,
            id,
            expected_sha256: self.sha256.clone(),
            expected_byte_length: self.bytes.len() as u64,
            content_type: self.mime,
            declarations: draft.declarations()?,
        })
    }
}
pub fn prepared_matches(
    work: &str,
    request: &CreateImageIntentRequest,
    r: &ImageIntentDocument,
) -> bool {
    r.work_id == work
        && r.id == request.id
        && r.operation_id == request.operation_id
        && r.request == *request
}
pub fn asset_matches(intent: &ImageIntentDocument, asset: &ImageAssetDocument) -> bool {
    asset.work_id == intent.work_id
        && asset.owner_id == intent.owner_id
        && asset.intent_id == intent.id
        && asset.verified_media.sha256 == intent.request.expected_sha256
        && asset.verified_media.byte_length == intent.request.expected_byte_length
        && asset.verified_media.content_type == intent.request.content_type
        && asset.declarations == intent.request.declarations
}
pub fn consistent_intent(r: &ImageIntentResponse) -> bool {
    match (&r.asset, r.state) {
        (Some(a), ImageIntentState::Admitted) => asset_matches(&r.intent, a),
        (None, ImageIntentState::Pending | ImageIntentState::Expired) => true,
        _ => false,
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Empty,
    Current,
    Replacement,
    Validating,
    Preparing,
    Prepared,
    Uploading,
    AwaitingAcknowledgement,
    Admitted,
    Saved,
    Failed,
    Expired,
    Unknown,
    Conflict,
    Loading,
    Forbidden,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Projection {
    pub stage: Stage,
    pub current: Option<WorkArtworkResponse>,
    pub current_url: Option<String>,
    pub current_media: Option<ImageVariantMetadata>,
    pub local: Option<LocalImage>,
    pub intent: Option<ImageIntentDocument>,
    pub intent_state: Option<ImageIntentState>,
    pub asset: Option<ImageAssetDocument>,
    pub problem: Option<Problem>,
    pub transferred: Option<u64>,
    pub compared_revision: Option<u64>,
    pub reported_revision: Option<u64>,
    pub operation: Option<String>,
}
impl Projection {
    fn file_problem(&self) -> Option<Problem> {
        self.problem
            .filter(|p| matches!(p, Problem::File | Problem::Dimensions | Problem::Decode))
    }

    pub fn file_ready(&self) -> bool {
        self.local.is_some() && self.file_problem().is_none()
    }

    pub fn can_prepare(&self) -> bool {
        self.file_ready()
            && (self.intent.is_none() || self.intent_state == Some(ImageIntentState::Expired))
    }

    pub fn can_upload(&self) -> bool {
        self.file_ready()
            && self.intent.is_some()
            && self.asset.is_none()
            && self.intent_state == Some(ImageIntentState::Pending)
    }

    /// Request progress cannot validate a rejected replacement file.
    pub fn start_request(&mut self, stage: Stage, operation: Option<String>) {
        self.operation = operation;
        self.stage = stage;
        self.problem = self.file_problem();
        self.transferred = None;
    }

    /// Keep a rejected selection guarded until a valid pick or explicit local discard.
    pub fn report_form_problem(&mut self, problem: Problem) {
        self.problem = self.file_problem().or(Some(problem));
    }

    pub fn replace_local(&mut self, file: LocalImage) -> Option<LocalImage> {
        let previous = self.local.replace(file);
        self.intent = None;
        self.intent_state = None;
        self.asset = None;
        self.problem = None;
        self.stage = Stage::Replacement;
        previous
    }

    pub fn discard_local(&mut self) -> Option<LocalImage> {
        let previous = self.local.take();
        self.intent = None;
        self.intent_state = None;
        self.asset = None;
        self.problem = None;
        self.stage = if self.current.as_ref().is_some_and(|r| r.artwork.is_some()) {
            Stage::Current
        } else {
            Stage::Empty
        };
        previous
    }

    /// Transfer retirement to the preview shell before forgetting its private Blob handle.
    pub fn take_current_preview(&mut self) -> Option<String> {
        self.current_media = None;
        self.current_url.take()
    }

    /// Only a confirmed server response replaces the current cover; local edits never do.
    pub fn apply(&self, reply: &super::Reply) -> Self {
        use super::Reply;
        let mut next = self.clone();
        next.problem = self.file_problem();
        next.transferred = None;
        match reply {
            Reply::Artwork(r) => {
                next.current = Some(r.clone());
                if next.compared_revision.is_some() {
                    next.reported_revision = Some(r.work_revision);
                }
                // The preview lifecycle must receive/revoke the old URL, even when this
                // reply changes or clears the selection before that effect runs.
                next.current_media = None;
                next.stage = if next.local.is_some() {
                    Stage::Replacement
                } else if r.artwork.is_some() {
                    Stage::Current
                } else {
                    Stage::Empty
                };
            }
            Reply::ImagePrepared(r) => {
                next.intent = Some(r.clone());
                next.intent_state = Some(ImageIntentState::Pending);
                next.asset = None;
                next.stage = Stage::Prepared;
            }
            Reply::ImageAdmitted(r) => {
                next.asset = Some(r.clone());
                next.intent_state = Some(ImageIntentState::Admitted);
                next.stage = Stage::Admitted;
            }
            Reply::ImageIntent(r) => {
                next.intent = Some(r.intent.clone());
                next.intent_state = Some(r.state);
                next.asset = r.asset.clone();
                next.stage = match r.state {
                    ImageIntentState::Pending => Stage::Prepared,
                    ImageIntentState::Admitted => Stage::Admitted,
                    ImageIntentState::Expired => Stage::Expired,
                };
            }
            _ => {}
        }
        next
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Refresh,
    Prepare,
    Upload,
    Select,
    Clear,
    DiscardLocal,
    Reconcile,
    Replay,
    ConfirmClear,
    CancelClear,
}
