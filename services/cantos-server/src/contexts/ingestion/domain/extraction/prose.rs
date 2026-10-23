//! UTF-8 text and Markdown: line labels, dash dialogue and headings are suggestions to review,
//! never resolved speakers or scenes.
use crate::contract::studio::{ImportBlockKind, ImportProblem};
use unicode_normalization::UnicodeNormalization;

use super::{problem, Builder, MAX_DECODED_BYTES};

pub(super) fn decode_text(bytes: &[u8], output: &mut Builder) -> Result<String, ImportProblem> {
    let text = std::str::from_utf8(bytes).map_err(|error| ImportProblem {
        code: "invalid_utf8".into(),
        offset: Some(error.valid_up_to() as u64),
    })?;
    for (offset, character) in text.char_indices() {
        if character.is_control() && !matches!(character, '\n' | '\r' | '\t') {
            return Err(ImportProblem {
                code: "unsupported_control_character".into(),
                offset: Some(offset as u64),
            });
        }
    }
    let without_bom = match text.strip_prefix('\u{feff}') {
        Some(text) => {
            output.warn("utf8_bom_removed", None);
            text
        }
        None => text,
    };
    if without_bom.contains('\r') {
        output.warn("line_endings_normalized", None);
    }
    let lines = without_bom.replace("\r\n", "\n").replace('\r', "\n");
    let normalized: String = lines.nfc().collect();
    if normalized != lines {
        output.warn("unicode_nfc_normalized", None);
    }
    if normalized.len() > MAX_DECODED_BYTES {
        return Err(problem("decoded_text_limit"));
    }
    Ok(normalized)
}

fn source_label(line: &str) -> Option<&str> {
    let (label, speech) = line.split_once(':')?;
    let label = label.trim();
    // This recognizes a lexical line label, not a character identity. Keep the full line and
    // require review: e.g. "Lưu ý: ..." can look exactly like a person's source label.
    if label.is_empty()
        || label.len() > 80
        || label.split_whitespace().count() > 4
        || speech.trim().is_empty()
        || !label.chars().next().is_some_and(char::is_uppercase)
        || !label
            .chars()
            .all(|character| character.is_alphabetic() || character == ' ' || character == '-')
    {
        return None;
    }
    Some(label)
}

pub(super) fn extract_prose(
    text: &str,
    markdown: bool,
    output: &mut Builder,
) -> Result<(), ImportProblem> {
    let mut paragraph = String::new();
    for line in text.split('\n') {
        let trimmed = line.trim();
        if trimmed != line && !trimmed.is_empty() {
            output.warn("paragraph_edge_whitespace_trimmed", None);
        }
        let heading = markdown && markdown_heading(trimmed).is_some();
        let label = source_label(trimmed);
        let dash_dialogue =
            trimmed.starts_with("- ") || trimmed.starts_with("– ") || trimmed.starts_with("— ");
        let bracket = trimmed.starts_with('[') && trimmed.ends_with(']');
        if trimmed.is_empty() || heading || label.is_some() || dash_dialogue || bracket {
            flush_paragraph(&mut paragraph, output)?;
        }
        if trimmed.is_empty() {
            continue;
        }
        if heading {
            let index = output.push(
                ImportBlockKind::SceneCue,
                trimmed.into(),
                None,
                markdown_heading(trimmed).map(str::to_owned),
                None,
            )?;
            output.warn("heading_scene_suggestion", Some(index));
        } else if let Some(label) = label {
            let index = output.push(
                ImportBlockKind::Dialogue,
                trimmed.into(),
                Some(label.into()),
                None,
                None,
            )?;
            output.warn("speaker_label_requires_review", Some(index));
        } else if dash_dialogue {
            let index = output.push(ImportBlockKind::Dialogue, trimmed.into(), None, None, None)?;
            output.warn("speaker_unknown", Some(index));
            if markdown {
                output.warn("markdown_list_or_dialogue_ambiguous", Some(index));
            }
        } else if bracket {
            // Brackets alone cannot tell a scene direction from spoken text or a link.
            let index =
                output.push(ImportBlockKind::Paragraph, trimmed.into(), None, None, None)?;
            output.warn("ambiguous_bracket_cue", Some(index));
        } else {
            if !paragraph.is_empty() {
                paragraph.push('\n');
            }
            paragraph.push_str(trimmed);
        }
        if markdown && (trimmed.contains(['*', '_', '`', '[', '|', '<'])) {
            output.warn("markdown_markup_preserved", None);
        }
    }
    flush_paragraph(&mut paragraph, output)
}

fn markdown_heading(line: &str) -> Option<&str> {
    let markers = line.bytes().take_while(|byte| *byte == b'#').count();
    if (1..=6).contains(&markers) && line.as_bytes().get(markers) == Some(&b' ') {
        let heading = line[markers + 1..].trim();
        if !heading.is_empty() {
            return Some(heading);
        }
    }
    None
}

fn flush_paragraph(text: &mut String, output: &mut Builder) -> Result<(), ImportProblem> {
    if !text.is_empty() {
        output.push(
            ImportBlockKind::Paragraph,
            std::mem::take(text),
            None,
            None,
            None,
        )?;
    }
    Ok(())
}
