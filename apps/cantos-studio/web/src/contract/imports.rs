//! Private source-import transport. None of these raw values establishes publication rights.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    Txt,
    Markdown,
    Docx,
    ScriptIr,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImportMetadata {
    pub operation_id: String,
    /// Inert display metadata, never a storage path.
    pub file_name: String,
    pub format: ImportFormat,
    pub reference: String,
    pub rights_holder: Option<String>,
    pub permission_evidence: Option<String>,
    pub usage_scope: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    pub metadata: ImportMetadata,
    /// Exact upload bytes; normalization only applies to the separate extraction.
    pub original_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImportResponse {
    pub id: String,
    pub imported_by: String,
    pub recorded_at: String,
    pub sha256: String,
    pub byte_len: u64,
    pub metadata: ImportMetadata,
    pub outcome: ImportOutcome,
    /// Exact UTF-8 source representation when available; binary originals use the download route.
    pub original_text: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ImportOutcome {
    Parsed { extraction: Extraction },
    Failed { error: ImportProblem },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    pub extractor_version: String,
    pub blocks: Vec<ImportBlock>,
    pub warnings: Vec<ImportWarning>,
    /// Complete normalized Script IR, only after admission through the existing validator.
    /// It is inspectable source content, never an automatic revision save.
    pub script_json: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImportBlock {
    pub index: u32,
    pub kind: ImportBlockKind,
    /// Source text including labels/markup for prose, or validated text for structured input.
    pub text: String,
    /// Lexical source label; prose labels require review and are never resolved cast identities.
    pub speaker: Option<String>,
    /// A source suggestion for prose; the validated scene ID for Script IR.
    pub scene: Option<String>,
    /// Closed Script IR cue kind when supplied by validated structured input.
    pub cue_kind: Option<ImportCueKind>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportBlockKind {
    Paragraph,
    Dialogue,
    Narration,
    SceneCue,
    SoundCue,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportCueKind {
    Ambience,
    Music,
    Sfx,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImportWarning {
    pub code: String,
    pub block: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImportProblem {
    pub code: String,
    /// UTF-8 byte offset in the input part when the parser can provide one.
    pub offset: Option<u64>,
}
