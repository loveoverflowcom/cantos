//! Deterministic bounded extraction of private manuscripts. No I/O, IDs, AI or revision saves.
//!
//! One entry point admits every format; each format keeps its own reader: plain text and
//! Markdown prose, DOCX (a bounded ZIP of WordprocessingML) and complete Script IR documents.
use crate::contract::studio::{Extraction, ImportFormat, ImportOutcome, ImportProblem};

mod builder;
mod docx;
mod prose;
mod script;

use builder::Builder;

pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_DECODED_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_BLOCKS: usize = 10_000;
pub const MAX_ZIP_ENTRIES: usize = 256;
pub const MAX_ZIP_RATIO: u64 = 100;
pub const MAX_XML_DEPTH: usize = 64;
pub const MAX_XML_EVENTS: usize = 100_000;
pub const EXTRACTOR_VERSION: &str = "cantos-import-1";

/// Admission is deliberately separate from persistence. A failure contains no partially ready
/// extraction, and neither branch changes the caller's immutable original bytes.
pub fn extract(bytes: &[u8], format: ImportFormat) -> ImportOutcome {
    let result = extract_bounded(bytes, format);
    match result {
        Ok(extraction) => ImportOutcome::Parsed { extraction },
        Err(error) => ImportOutcome::Failed { error },
    }
}

pub(super) fn problem(code: &str) -> ImportProblem {
    ImportProblem {
        code: code.into(),
        offset: None,
    }
}

fn extract_bounded(bytes: &[u8], format: ImportFormat) -> Result<Extraction, ImportProblem> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(problem("source_too_large"));
    }
    if bytes.is_empty() {
        return Err(problem("empty_source"));
    }
    if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0]) {
        return Err(problem("legacy_or_encrypted_word"));
    }
    let zip = bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08");
    if zip != (format == ImportFormat::Docx) {
        return Err(problem("format_mismatch"));
    }
    match format {
        ImportFormat::Txt | ImportFormat::Markdown => {
            let mut output = Builder::new();
            let text = prose::decode_text(bytes, &mut output)?;
            prose::extract_prose(&text, format == ImportFormat::Markdown, &mut output)?;
            output.finish(None)
        }
        ImportFormat::Docx => docx::extract_docx(bytes),
        ImportFormat::ScriptIr => script::extract_script(bytes),
    }
}
