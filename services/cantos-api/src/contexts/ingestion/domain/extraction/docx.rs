//! DOCX: a bounded ZIP preflight, then the main WordprocessingML part read as paragraphs.
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};

use cantos_contracts::{Extraction, ImportBlockKind, ImportProblem};
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use unicode_normalization::UnicodeNormalization;

use super::prose::{decode_text, extract_prose};
use super::{
    problem, Builder, MAX_BLOCKS, MAX_DECODED_BYTES, MAX_XML_DEPTH, MAX_XML_EVENTS,
    MAX_ZIP_ENTRIES, MAX_ZIP_RATIO,
};

const WORD_NS: &[u8] = b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const STRICT_WORD_NS: &[u8] = b"http://purl.oclc.org/ooxml/wordprocessingml/main";
const MAIN_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const MACRO_MAIN_TYPE: &str = "application/vnd.ms-word.document.macroEnabled.main+xml";

pub(super) fn extract_docx(bytes: &[u8]) -> Result<Extraction, ImportProblem> {
    preflight_zip(bytes)?;
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| problem("malformed_archive"))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(problem("archive_entry_limit"));
    }
    let mut total = 0_u64;
    let mut names = BTreeSet::new();
    let mut output = Builder::new();
    let mut document = None;
    let mut main_type = false;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| problem("malformed_or_encrypted_archive"))?;
        let name = entry.name().to_owned();
        if !safe_zip_name(&name) {
            return Err(problem("unsafe_archive_path"));
        }
        if !names.insert(name.clone()) {
            return Err(problem("duplicate_archive_entry"));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(problem("archive_symlink_forbidden"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| problem("archive_expansion_limit"))?;
        if total > MAX_DECODED_BYTES as u64 {
            return Err(problem("archive_expansion_limit"));
        }
        if entry.size() > entry.compressed_size().max(1).saturating_mul(MAX_ZIP_RATIO) {
            return Err(problem("archive_ratio_limit"));
        }
        if entry.is_dir() {
            continue;
        }
        // Read every entry through the bound to verify CRC and actual expansion. Nothing is
        // extracted to a filesystem, and no embedded object's bytes are executed.
        let mut contents = Vec::new();
        entry
            .by_ref()
            .take(MAX_DECODED_BYTES as u64 + 1)
            .read_to_end(&mut contents)
            .map_err(|_| problem("malformed_archive_entry"))?;
        if contents.len() != entry.size() as usize || contents.len() > MAX_DECODED_BYTES {
            return Err(problem("archive_expansion_limit"));
        }
        if name.ends_with("vbaProject.bin") {
            output.warn("macros_ignored", None);
        }
        if name.starts_with("word/embeddings/") {
            output.warn("embedded_objects_ignored", None);
        }
        if name.starts_with("word/media/") {
            output.warn("docx_images_omitted", None);
        }
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let xml = decode_text(&contents, &mut output)?;
            let facts = inspect_xml(&xml, name == "word/document.xml", &mut output)?;
            if name == "[Content_Types].xml" {
                main_type = facts.main_document_type;
            }
            if name == "word/document.xml" {
                document = Some(facts.paragraphs);
            }
            if matches!(
                name.as_str(),
                "word/comments.xml" | "word/footnotes.xml" | "word/endnotes.xml"
            ) {
                output.warn("docx_annotations_omitted", None);
            }
            if name.starts_with("word/header") || name.starts_with("word/footer") {
                output.warn("docx_headers_footers_omitted", None);
            }
        }
    }
    if !main_type {
        return Err(problem("docx_main_content_type_missing"));
    }
    let paragraphs = document.ok_or_else(|| problem("docx_document_missing"))?;
    output.warn("docx_formatting_omitted", None);
    for paragraph in paragraphs {
        if paragraph.text.trim().is_empty() {
            continue;
        }
        let normalized: String = paragraph.text.nfc().collect();
        if normalized != paragraph.text {
            output.warn("unicode_nfc_normalized", None);
        }
        if paragraph.heading {
            let title = normalized.trim().to_owned();
            let index = output.push(
                ImportBlockKind::SceneCue,
                title.clone(),
                None,
                Some(title),
                None,
            )?;
            output.warn("heading_scene_suggestion", Some(index));
        } else {
            if normalized.contains('\n') {
                output.warn("docx_soft_breaks_segmented", None);
            }
            extract_prose(&normalized, false, &mut output)?;
        }
    }
    output.finish(None)
}

/// Bound the directory before the ZIP dependency allocates from its declared entry count. The
/// 1 MiB manuscript limit cannot need ZIP64; rejecting that extension also closes its wider
/// attacker-controlled count/offset fields. The standard library reader still owns ZIP/CRC
/// decoding; this preflight is only a small resource and path admission barrier.
fn preflight_zip(bytes: &[u8]) -> Result<(), ImportProblem> {
    let end = bytes
        .windows(4)
        .rposition(|signature| signature == b"PK\x05\x06")
        .ok_or_else(|| problem("malformed_archive"))?;
    if end + 22 > bytes.len() {
        return Err(problem("malformed_archive"));
    }
    let comment = zip_u16(bytes, end + 20)? as usize;
    if end + 22 + comment != bytes.len() {
        return Err(problem("malformed_archive"));
    }
    if zip_u16(bytes, end + 4)? != 0 || zip_u16(bytes, end + 6)? != 0 {
        return Err(problem("archive_multidisk_unsupported"));
    }
    let count = zip_u16(bytes, end + 10)? as usize;
    let directory_size = zip_u32(bytes, end + 12)?;
    let directory_start = zip_u32(bytes, end + 16)?;
    if count == u16::MAX as usize
        || directory_size == u32::MAX
        || directory_start == u32::MAX
        || (end >= 20 && bytes.get(end - 20..end - 16) == Some(b"PK\x06\x07".as_slice()))
    {
        return Err(problem("archive_zip64_unsupported"));
    }
    if count > MAX_ZIP_ENTRIES {
        return Err(problem("archive_entry_limit"));
    }
    if zip_u16(bytes, end + 8)? as usize != count
        || directory_start as u64 + directory_size as u64 != end as u64
    {
        return Err(problem("malformed_archive"));
    }
    let mut position = directory_start as usize;
    let mut names = BTreeSet::new();
    let mut expansion = 0_u64;
    for _ in 0..count {
        if bytes.get(position..position.saturating_add(4)) != Some(b"PK\x01\x02".as_slice()) {
            return Err(problem("malformed_archive"));
        }
        let compressed = zip_u32(bytes, position + 20)?;
        let decoded = zip_u32(bytes, position + 24)?;
        let local_offset = zip_u32(bytes, position + 42)?;
        if compressed == u32::MAX || decoded == u32::MAX || local_offset == u32::MAX {
            return Err(problem("archive_zip64_unsupported"));
        }
        if compressed as usize > bytes.len() || local_offset as usize >= bytes.len() {
            return Err(problem("malformed_archive_entry"));
        }
        if zip_u16(bytes, position + 8)? & 1 != 0 {
            return Err(problem("malformed_or_encrypted_archive"));
        }
        expansion += decoded as u64;
        if expansion > MAX_DECODED_BYTES as u64 {
            return Err(problem("archive_expansion_limit"));
        }
        if decoded as u64 > (compressed as u64).max(1).saturating_mul(MAX_ZIP_RATIO) {
            return Err(problem("archive_ratio_limit"));
        }
        let name_len = zip_u16(bytes, position + 28)? as usize;
        let extra_len = zip_u16(bytes, position + 30)? as usize;
        let comment_len = zip_u16(bytes, position + 32)? as usize;
        let name_bytes = bytes
            .get(position + 46..position + 46 + name_len)
            .ok_or_else(|| problem("malformed_archive"))?;
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| problem("archive_filename_encoding_unsupported"))?;
        if !safe_zip_name(name) {
            return Err(problem("unsafe_archive_path"));
        }
        if !names.insert(name) {
            return Err(problem("duplicate_archive_entry"));
        }
        position += 46 + name_len + extra_len + comment_len;
        if position > end {
            return Err(problem("malformed_archive"));
        }
    }
    if position != end {
        return Err(problem("malformed_archive"));
    }
    Ok(())
}

fn zip_u16(bytes: &[u8], offset: usize) -> Result<u16, ImportProblem> {
    let value: [u8; 2] = bytes
        .get(offset..offset.saturating_add(2))
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| problem("malformed_archive"))?;
    Ok(u16::from_le_bytes(value))
}

fn zip_u32(bytes: &[u8], offset: usize) -> Result<u32, ImportProblem> {
    let value: [u8; 4] = bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| problem("malformed_archive"))?;
    Ok(u32::from_le_bytes(value))
}

fn safe_zip_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains(['\\', ':'])
        && !name.chars().any(char::is_control)
        && name
            .trim_end_matches('/')
            .split('/')
            .all(|component| !matches!(component, "" | "." | ".."))
}

#[derive(Default)]
struct XmlParagraph {
    text: String,
    heading: bool,
}

#[derive(Default)]
struct XmlFacts {
    paragraphs: Vec<XmlParagraph>,
    main_document_type: bool,
}

fn word_namespace(namespace: &ResolveResult<'_>) -> bool {
    matches!(namespace, ResolveResult::Bound(value) if value.as_ref() == WORD_NS || value.as_ref() == STRICT_WORD_NS)
}

fn attributes(element: &BytesStart<'_>) -> Result<BTreeMap<String, String>, ImportProblem> {
    let mut output = BTreeMap::new();
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|_| problem("malformed_xml"))?;
        let key = std::str::from_utf8(attribute.key.local_name().as_ref())
            .map_err(|_| problem("malformed_xml"))?
            .to_owned();
        let value = attribute
            .unescape_value()
            .map_err(|_| problem("xml_entity_forbidden"))?;
        if value.chars().any(|character| character.is_control()) {
            return Err(problem("unsupported_control_character"));
        }
        if output.insert(key, value.into_owned()).is_some() {
            return Err(problem("ambiguous_xml_attribute"));
        }
    }
    Ok(output)
}

fn inspect_xml(xml: &str, document: bool, output: &mut Builder) -> Result<XmlFacts, ImportProblem> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().check_end_names = true;
    reader.config_mut().allow_unmatched_ends = false;
    let mut facts = XmlFacts::default();
    let mut depth = 0_usize;
    let mut events = 0_usize;
    let mut roots = 0_usize;
    let mut body_depth = None;
    let mut bodies = 0_usize;
    let mut paragraph = None::<XmlParagraph>;
    let mut text_depth = None;
    loop {
        events += 1;
        if events > MAX_XML_EVENTS {
            return Err(problem("xml_event_limit"));
        }
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| problem("malformed_xml"))?;
        let word = word_namespace(&namespace);
        let empty = matches!(&event, Event::Empty(_));
        if matches!(namespace, ResolveResult::Unknown(_)) {
            return Err(problem("xml_namespace_unbound"));
        }
        match event {
            Event::Start(element) | Event::Empty(element) => {
                if depth == 0 {
                    roots += 1;
                    if roots > 1 {
                        return Err(problem("malformed_xml"));
                    }
                    if document && (!word || element.local_name().as_ref() != b"document") {
                        return Err(problem("docx_document_namespace_invalid"));
                    }
                }
                if !empty {
                    depth += 1;
                    if depth > MAX_XML_DEPTH {
                        return Err(problem("xml_depth_limit"));
                    }
                }
                let name = element.local_name();
                let attributes = attributes(&element)?;
                if name.as_ref() == b"Override"
                    && attributes.get("PartName").map(String::as_str) == Some("/word/document.xml")
                {
                    let content_type = attributes.get("ContentType").map(String::as_str);
                    facts.main_document_type =
                        matches!(content_type, Some(MAIN_TYPE | MACRO_MAIN_TYPE));
                    if content_type == Some(MACRO_MAIN_TYPE) {
                        output.warn("macros_ignored", None);
                    }
                }
                if name.as_ref() == b"Relationship"
                    && attributes.get("TargetMode").map(String::as_str) == Some("External")
                {
                    output.warn("external_relationships_ignored", None);
                }
                if word {
                    inspect_word_element(name.as_ref(), &attributes, output)?;
                    if document {
                        match name.as_ref() {
                            b"body" => {
                                bodies += 1;
                                if depth != 2 || bodies > 1 || empty {
                                    return Err(problem("docx_body_invalid"));
                                }
                                body_depth = Some(depth);
                            }
                            b"p" => {
                                if body_depth.is_none() {
                                    return Err(problem("docx_paragraph_outside_body"));
                                }
                                if paragraph.is_some() {
                                    return Err(problem("docx_nested_paragraph"));
                                }
                                paragraph = Some(XmlParagraph::default());
                                if empty {
                                    paragraph = None;
                                }
                            }
                            b"pStyle" => {
                                if let Some(paragraph) = &mut paragraph {
                                    paragraph.heading =
                                        attributes.get("val").is_some_and(|value| {
                                            value.to_ascii_lowercase().starts_with("heading")
                                        });
                                }
                            }
                            b"t" if !empty => {
                                if paragraph.is_none() {
                                    return Err(problem("docx_text_outside_paragraph"));
                                }
                                if text_depth.is_some() {
                                    return Err(problem("docx_nested_text"));
                                }
                                text_depth = Some(depth);
                            }
                            b"br" | b"cr" => {
                                if let Some(paragraph) = &mut paragraph {
                                    paragraph.text.push('\n');
                                }
                            }
                            b"tab" => {
                                if let Some(paragraph) = &mut paragraph {
                                    paragraph.text.push('\t');
                                }
                            }
                            b"noBreakHyphen" | b"softHyphen" => {
                                if let Some(paragraph) = &mut paragraph {
                                    paragraph.text.push(if name.as_ref() == b"noBreakHyphen" {
                                        '\u{2011}'
                                    } else {
                                        '\u{00ad}'
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Event::End(element) => {
                if depth == 0 {
                    return Err(problem("malformed_xml"));
                }
                if text_depth == Some(depth) {
                    text_depth = None;
                }
                if body_depth == Some(depth) {
                    body_depth = None;
                }
                if document && word && element.local_name().as_ref() == b"p" {
                    if let Some(paragraph) = paragraph.take() {
                        if facts.paragraphs.len() >= MAX_BLOCKS {
                            return Err(problem("block_count_limit"));
                        }
                        facts.paragraphs.push(paragraph);
                    }
                }
                depth -= 1;
            }
            Event::Text(text) => {
                let text = text.xml_content().map_err(|_| problem("invalid_utf8"))?;
                if depth == 0 && !text.trim().is_empty() {
                    return Err(problem("malformed_xml"));
                }
                if text_depth.is_some() {
                    if let Some(paragraph) = &mut paragraph {
                        paragraph.text.push_str(&text);
                    }
                }
            }
            Event::GeneralRef(reference) => {
                if depth == 0 {
                    return Err(problem("malformed_xml"));
                }
                let character = reference
                    .resolve_char_ref()
                    .map_err(|_| problem("xml_entity_forbidden"))?;
                let resolved = match character {
                    Some(character) => character.to_string(),
                    None => {
                        let name = reference
                            .decode()
                            .map_err(|_| problem("xml_entity_forbidden"))?;
                        quick_xml::escape::resolve_predefined_entity(&name)
                            .ok_or_else(|| problem("xml_entity_forbidden"))?
                            .to_owned()
                    }
                };
                if resolved.chars().any(|character| {
                    character.is_control() && !matches!(character, '\n' | '\t' | '\r')
                }) {
                    return Err(problem("unsupported_control_character"));
                }
                if text_depth.is_some() {
                    if let Some(paragraph) = &mut paragraph {
                        paragraph.text.push_str(&resolved);
                    }
                }
            }
            Event::CData(_) => return Err(problem("docx_cdata_unsupported")),
            Event::DocType(_) => return Err(problem("xml_doctype_forbidden")),
            Event::Decl(declaration) => {
                if depth != 0 || roots != 0 {
                    return Err(problem("malformed_xml"));
                }
                if let Some(encoding) = declaration.encoding() {
                    let encoding = encoding.map_err(|_| problem("malformed_xml"))?;
                    if !encoding.eq_ignore_ascii_case(b"utf-8") {
                        return Err(problem("unsupported_xml_encoding"));
                    }
                }
            }
            Event::PI(_) => output.warn("xml_processing_instruction_ignored", None),
            Event::Comment(_) => {}
            Event::Eof => {
                if depth != 0 || roots != 1 || paragraph.is_some() {
                    return Err(problem("malformed_xml"));
                }
                if document && bodies != 1 {
                    return Err(problem("docx_body_missing"));
                }
                break;
            }
        }
    }
    Ok(facts)
}

fn inspect_word_element(
    name: &[u8],
    attributes: &BTreeMap<String, String>,
    output: &mut Builder,
) -> Result<(), ImportProblem> {
    match name {
        b"ins" | b"del" | b"delText" | b"moveFrom" | b"moveTo" | b"rPrChange" | b"pPrChange"
        | b"tblPrChange" | b"sectPrChange" => {
            return Err(problem("docx_tracked_changes_require_choice"));
        }
        b"rFonts" => {
            if attributes.values().any(|font| {
                let font = font.to_ascii_lowercase();
                font.starts_with(".vn") || font.starts_with("vni-")
            }) {
                return Err(problem("docx_legacy_encoding_requires_choice"));
            }
        }
        b"tbl" => output.warn("docx_tables_flattened", None),
        b"txbxContent" => output.warn("docx_textboxes_flattened", None),
        b"instrText" => output.warn("docx_field_instructions_omitted", None),
        b"drawing" | b"pict" => output.warn("docx_images_omitted", None),
        b"altChunk" => output.warn("docx_external_content_omitted", None),
        b"sym" => output.warn("docx_symbols_omitted", None),
        _ => {}
    }
    Ok(())
}
