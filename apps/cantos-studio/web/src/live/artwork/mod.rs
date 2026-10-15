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
use cantos_contracts::*;
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
        "image/png" => Ok(ImageMime::Png),
        "image/jpeg" => Ok(ImageMime::Jpeg),
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
pub fn mime_name(mime: ImageMime) -> &'static str {
    match mime {
        ImageMime::Png => "image/png",
        ImageMime::Jpeg => "image/jpeg",
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
    pub asset: Option<ImageAssetDocument>,
    pub problem: Option<Problem>,
    pub transferred: Option<u64>,
    pub compared_revision: Option<u64>,
    pub reported_revision: Option<u64>,
    pub operation: Option<String>,
}
impl Projection {
    pub fn file_ready(&self) -> bool {
        self.local.is_some()
            && !matches!(
                self.problem,
                Some(Problem::File | Problem::Dimensions | Problem::Decode)
            )
    }

    /// Only a confirmed server response replaces the current cover; local edits never do.
    pub fn apply(&self, reply: &super::Reply) -> Self {
        use super::Reply;
        let mut next = self.clone();
        next.problem = None;
        next.transferred = None;
        match reply {
            Reply::Artwork(r) => {
                next.current = Some(r.clone());
                if next.compared_revision.is_some() {
                    next.reported_revision = Some(r.work_revision);
                }
                next.current_url = None;
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
                next.asset = None;
                next.stage = Stage::Prepared;
            }
            Reply::ImageAdmitted(r) => {
                next.asset = Some(r.clone());
                next.stage = Stage::Admitted;
            }
            Reply::ImageIntent(r) => {
                next.intent = Some(r.intent.clone());
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
