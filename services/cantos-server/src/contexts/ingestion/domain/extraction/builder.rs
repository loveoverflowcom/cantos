//! Accumulates admitted blocks and de-duplicated warnings under the shared extraction bounds.
use std::collections::BTreeSet;

use cantos_contracts::{
    Extraction, ImportBlock, ImportBlockKind, ImportCueKind, ImportProblem, ImportWarning,
};

use super::{problem, EXTRACTOR_VERSION, MAX_BLOCKS, MAX_DECODED_BYTES};

pub(super) struct Builder {
    blocks: Vec<ImportBlock>,
    warnings: Vec<ImportWarning>,
    decoded_bytes: usize,
    warning_keys: BTreeSet<(String, Option<u32>)>,
}

impl Builder {
    pub(super) fn new() -> Self {
        Self {
            blocks: Vec::new(),
            warnings: Vec::new(),
            decoded_bytes: 0,
            warning_keys: BTreeSet::new(),
        }
    }

    pub(super) fn warn(&mut self, code: &str, block: Option<u32>) {
        if self.warning_keys.insert((code.into(), block)) {
            self.warnings.push(ImportWarning {
                code: code.into(),
                block,
            });
        }
    }

    pub(super) fn push(
        &mut self,
        kind: ImportBlockKind,
        text: String,
        speaker: Option<String>,
        scene: Option<String>,
        cue_kind: Option<ImportCueKind>,
    ) -> Result<u32, ImportProblem> {
        for value in [Some(&text), speaker.as_ref(), scene.as_ref()]
            .into_iter()
            .flatten()
        {
            if value
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
            {
                return Err(problem("unsupported_control_character"));
            }
        }
        if self.blocks.len() >= MAX_BLOCKS {
            return Err(problem("block_count_limit"));
        }
        self.decoded_bytes += text.len()
            + speaker.as_ref().map_or(0, String::len)
            + scene.as_ref().map_or(0, String::len);
        if self.decoded_bytes > MAX_DECODED_BYTES {
            return Err(problem("decoded_text_limit"));
        }
        let index = self.blocks.len() as u32;
        self.blocks.push(ImportBlock {
            index,
            kind,
            text,
            speaker,
            scene,
            cue_kind,
        });
        Ok(index)
    }

    pub(super) fn finish(self, script_json: Option<String>) -> Result<Extraction, ImportProblem> {
        if self.blocks.is_empty() {
            return Err(problem("empty_source"));
        }
        Ok(Extraction {
            extractor_version: EXTRACTOR_VERSION.into(),
            blocks: self.blocks,
            warnings: self.warnings,
            script_json,
        })
    }
}
