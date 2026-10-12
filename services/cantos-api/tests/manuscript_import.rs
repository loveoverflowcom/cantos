use std::io::{Cursor, Write};

use cantos_api::contexts::ingestion::{
    extract, MAX_BLOCKS, MAX_SOURCE_BYTES, MAX_XML_DEPTH, MAX_XML_EVENTS, MAX_ZIP_ENTRIES,
};
use cantos_api::script_ir::read_canonical_script;
use cantos_contracts::{
    Extraction, ImportBlockKind, ImportCueKind, ImportFormat, ImportOutcome, ImportProblem,
};
use unicode_normalization::UnicodeNormalization;
use zip::write::SimpleFileOptions;

const TXT: &[u8] = include_bytes!("../../../packages/contracts/fixtures/manuscript/chapter-vi.txt");
const MD: &[u8] = include_bytes!("../../../packages/contracts/fixtures/manuscript/chapter-vi.md");
const IR: &[u8] =
    include_bytes!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const DOCUMENT_START: &str = r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#;
const DOCUMENT_END: &str = "</w:body></w:document>";

fn parsed(bytes: &[u8], format: ImportFormat) -> Extraction {
    match extract(bytes, format) {
        ImportOutcome::Parsed { extraction } => extraction,
        other => panic!("expected parsed source, found {other:?}"),
    }
}

fn failed(bytes: &[u8], format: ImportFormat, code: &str, offset: Option<u64>) {
    assert_eq!(
        extract(bytes, format),
        ImportOutcome::Failed {
            error: ImportProblem {
                code: code.into(),
                offset
            }
        }
    );
}

fn warnings(extraction: &Extraction) -> Vec<(&str, Option<u32>)> {
    extraction
        .warnings
        .iter()
        .map(|warning| (warning.code.as_str(), warning.block))
        .collect()
}

fn archive(entries: &[(&str, &[u8])], compression: zip::CompressionMethod) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(compression),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn docx(xml: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let mut entries = vec![
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes()),
        ("word/document.xml", xml.as_bytes()),
    ];
    entries.extend_from_slice(extra);
    archive(&entries, zip::CompressionMethod::Stored)
}

fn document(body: &str) -> String {
    format!("{DOCUMENT_START}{body}{DOCUMENT_END}")
}

fn central_offset(bytes: &[u8]) -> usize {
    bytes
        .windows(4)
        .position(|bytes| bytes == b"PK\x01\x02")
        .unwrap()
}

#[test]
fn independent_vietnamese_oracle_preserves_source_labels_and_unknowns() {
    let expected: Extraction = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/manuscript/chapter-vi.expected.json"
    ))
    .unwrap();
    assert_eq!(parsed(TXT, ImportFormat::Txt), expected);
}

#[test]
fn markdown_is_inert_and_headings_and_lists_remain_reviewable_suggestions() {
    let result = parsed(MD, ImportFormat::Markdown);
    assert_eq!(result.blocks.len(), 7);
    let first = &result.blocks[0];
    assert_eq!(first.kind, ImportBlockKind::SceneCue);
    assert_eq!(first.text, "# Bến sông lúc bình minh");
    assert_eq!(first.scene.as_deref(), Some("Bến sông lúc bình minh"));
    assert_eq!(first.speaker, None);
    assert_eq!(result.blocks[1].kind, ImportBlockKind::Paragraph);
    assert_eq!(result.blocks[1].speaker, None);
    assert_eq!(result.blocks[2].speaker.as_deref(), Some("An"));
    assert_eq!(result.blocks[3].speaker.as_deref(), Some("Chú Lâm"));
    assert_eq!(result.blocks[4].speaker, None);
    assert_eq!(result.blocks[5].cue_kind, None);
    assert_eq!(
        result.blocks[6].text,
        "<script>alert(\"source stays inert\")</script>"
    );
    assert_eq!(
        warnings(&result),
        [
            ("heading_scene_suggestion", Some(0)),
            ("markdown_markup_preserved", None),
            ("speaker_label_requires_review", Some(2)),
            ("speaker_label_requires_review", Some(3)),
            ("speaker_unknown", Some(4)),
            ("markdown_list_or_dialogue_ambiguous", Some(4)),
            ("ambiguous_bracket_cue", Some(5)),
        ]
    );
    assert_eq!(result.script_json, None);
}

#[test]
fn normalization_is_derived_and_distinct_vietnamese_orthography_stays_distinct() {
    let nfd: String = "Người dẫn chuyện\r\n\r\nhoà hòa thúy thuý Đ Ð…"
        .nfd()
        .collect();
    let source = format!("\u{feff}{nfd}").into_bytes();
    let original = source.clone();
    let result = parsed(&source, ImportFormat::Txt);
    assert_eq!(source, original);
    assert_eq!(result.blocks[0].text, "Người dẫn chuyện");
    assert_eq!(result.blocks[0].kind, ImportBlockKind::Paragraph);
    assert_eq!(result.blocks[0].speaker, None);
    assert_eq!(result.blocks[1].text, "hoà hòa thúy thuý Đ Ð…");
    assert_eq!(
        warnings(&result),
        [
            ("utf8_bom_removed", None),
            ("line_endings_normalized", None),
            ("unicode_nfc_normalized", None),
        ]
    );
}

#[test]
fn invalid_utf8_control_and_empty_inputs_have_exact_byte_diagnostics() {
    failed(&[b'a', 0xff], ImportFormat::Txt, "invalid_utf8", Some(1));
    failed(
        b"x\0y",
        ImportFormat::Txt,
        "unsupported_control_character",
        Some(1),
    );
    failed(
        &[0x4e, 0x67, 0xfd, 0xf5, 0xcc, 0x69],
        ImportFormat::Txt,
        "invalid_utf8",
        Some(2),
    );
    for format in [
        ImportFormat::Txt,
        ImportFormat::Markdown,
        ImportFormat::Docx,
        ImportFormat::ScriptIr,
    ] {
        failed(b"", format, "empty_source", None);
    }
    failed(b" \n\r\t", ImportFormat::Txt, "empty_source", None);
}

#[test]
fn source_and_block_limits_accept_the_boundary_and_reject_the_next_value() {
    let text = vec![b'x'; MAX_SOURCE_BYTES];
    assert_eq!(
        parsed(&text, ImportFormat::Txt).blocks[0].text.len(),
        MAX_SOURCE_BYTES
    );
    failed(
        &vec![b'x'; MAX_SOURCE_BYTES + 1],
        ImportFormat::Txt,
        "source_too_large",
        None,
    );
    let blocks = "x\n\n".repeat(MAX_BLOCKS);
    assert_eq!(
        parsed(blocks.as_bytes(), ImportFormat::Txt).blocks.len(),
        MAX_BLOCKS
    );
    failed(
        format!("{blocks}x").as_bytes(),
        ImportFormat::Txt,
        "block_count_limit",
        None,
    );
    let labelled = "An: Ngày mai mình gặp lại.\n".repeat(MAX_BLOCKS);
    let result = parsed(labelled.as_bytes(), ImportFormat::Txt);
    assert_eq!(result.blocks.len(), MAX_BLOCKS);
    assert_eq!(result.warnings.len(), MAX_BLOCKS);
    assert!(result.warnings.iter().enumerate().all(|(index, warning)| {
        warning.code == "speaker_label_requires_review" && warning.block == Some(index as u32)
    }));
}

#[test]
fn type_sniffing_rejects_container_spoofing_and_legacy_word() {
    let valid = docx(&document("<w:p><w:r><w:t>Xin chào.</w:t></w:r></w:p>"), &[]);
    for format in [
        ImportFormat::Txt,
        ImportFormat::Markdown,
        ImportFormat::ScriptIr,
    ] {
        failed(&valid, format, "format_mismatch", None);
    }
    failed(
        b"A plain text document",
        ImportFormat::Docx,
        "format_mismatch",
        None,
    );
    failed(
        &[0xd0, 0xcf, 0x11, 0xe0, 0, 0],
        ImportFormat::Docx,
        "legacy_or_encrypted_word",
        None,
    );
}

#[test]
fn structured_import_preserves_full_document_and_projects_validated_cue_anchor_order() {
    let result = parsed(IR, ImportFormat::ScriptIr);
    assert_eq!(result.blocks.len(), 8);
    assert_eq!(
        result
            .blocks
            .iter()
            .map(|block| block.kind)
            .collect::<Vec<_>>(),
        [
            ImportBlockKind::SceneCue,
            ImportBlockKind::SoundCue,
            ImportBlockKind::Narration,
            ImportBlockKind::Dialogue,
            ImportBlockKind::Dialogue,
            ImportBlockKind::SceneCue,
            ImportBlockKind::Dialogue,
            ImportBlockKind::SoundCue,
        ]
    );
    assert_eq!(result.blocks[1].cue_kind, Some(ImportCueKind::Ambience));
    assert_eq!(
        result.blocks[2].speaker.as_deref(),
        Some("Người dẫn chuyện")
    );
    assert_eq!(result.blocks[3].speaker.as_deref(), Some("An"));
    assert_eq!(result.blocks[7].cue_kind, Some(ImportCueKind::Sfx));
    assert_eq!(result.blocks[7].scene.as_deref(), Some("scene-02"));
    let canonical = result.script_json.unwrap();
    let admitted = read_canonical_script(canonical.as_bytes()).unwrap();
    assert_eq!(admitted.export_bytes(), canonical.as_bytes());
    assert!(canonical.contains("pronunciation_overrides"));
    assert!(canonical.contains("rights-asset-demo"));
}

#[test]
fn structured_rejections_use_the_existing_reader_and_never_return_partial_blocks() {
    let cases = [
        (br#"{}"#.as_slice(), "missing_script_ir_version"),
        (
            br#"{"schema_version":"9.0.0"}"#.as_slice(),
            "unsupported_script_ir_version",
        ),
        (b"{broken".as_slice(), "invalid_script_ir"),
        (
            include_bytes!(
                "../../../packages/contracts/fixtures/script-ir/0.1.0/reject/unknown-field.json"
            )
            .as_slice(),
            "invalid_script_ir",
        ),
        (
            include_bytes!(
                "../../../packages/contracts/fixtures/script-ir/0.1.0/reject/unknown-speaker.json"
            )
            .as_slice(),
            "script_ir_semantics_invalid",
        ),
    ];
    for (bytes, code) in cases {
        failed(bytes, ImportFormat::ScriptIr, code, None);
    }
    let deeply_nested = format!(
        "{{\"schema_version\":\"0.1.0\",\"payload\":{}0{}}}",
        "[".repeat(140),
        "]".repeat(140)
    );
    failed(
        deeply_nested.as_bytes(),
        ImportFormat::ScriptIr,
        "invalid_script_ir",
        None,
    );
    let escaped_control = std::str::from_utf8(IR).unwrap().replace(
        "Ngày mai, mình có diễn tiếp không?",
        "Ngày mai\\u0000, mình có diễn tiếp không?",
    );
    failed(
        escaped_control.as_bytes(),
        ImportFormat::ScriptIr,
        "script_ir_semantics_invalid",
        None,
    );
}

#[test]
fn docx_word_hyphen_elements_preserve_word_boundaries_and_symbols_have_a_loss_note() {
    let xml = document(
        r#"<w:p><w:r><w:t>Hoa</w:t><w:noBreakHyphen/><w:t>Bình và ánh</w:t><w:softHyphen/><w:t>đèn</w:t><w:sym w:font="Wingdings" w:char="F0FC"/></w:r></w:p>"#,
    );
    let result = parsed(&docx(&xml, &[]), ImportFormat::Docx);
    assert_eq!(result.blocks[0].text, "Hoa\u{2011}Bình và ánh\u{00ad}đèn");
    assert!(warnings(&result).contains(&("docx_symbols_omitted", None)));
}

#[test]
fn docx_extracts_vietnamese_paragraphs_breaks_entities_and_explicit_labels() {
    let xml = document(
        r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Bến sông</w:t></w:r></w:p><w:p><w:r><w:t>An: Người</w:t><w:br/><w:t>dẫn chuyện &amp; Minh…</w:t></w:r></w:p><w:p><w:r><w:t>e&#x301; hoà hòa</w:t></w:r></w:p>"#,
    );
    let result = parsed(&docx(&xml, &[]), ImportFormat::Docx);
    assert_eq!(result.blocks[0].kind, ImportBlockKind::SceneCue);
    assert_eq!(result.blocks[0].text, "Bến sông");
    assert_eq!(result.blocks[1].text, "An: Người");
    assert_eq!(result.blocks[1].speaker.as_deref(), Some("An"));
    assert_eq!(result.blocks[2].text, "dẫn chuyện & Minh…");
    assert_eq!(result.blocks[2].speaker, None);
    assert_eq!(result.blocks[3].text, "é hoà hòa");
    assert!(warnings(&result).contains(&("unicode_nfc_normalized", None)));
    assert_eq!(result.script_json, None);
}

#[test]
fn docx_macro_external_relationship_and_loss_notes_are_visible_without_loading_resources() {
    let relationships = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Target="file:///private/secret" TargetMode="External" Type="anything"/></Relationships>"#;
    let comments = br#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment>Private note</w:comment></w:comments>"#;
    let xml = document(
        r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Trà của Chú Lâm.</w:t><w:instrText>INCLUDETEXT https://example.invalid/secret</w:instrText></w:r></w:p></w:tc></w:tr></w:tbl><w:altChunk/>"#,
    );
    let source = docx(
        &xml,
        &[
            ("word/_rels/document.xml.rels", relationships),
            ("word/vbaProject.bin", b"inert macro bytes"),
            ("word/embeddings/object.bin", b"inert object bytes"),
            ("word/comments.xml", comments),
        ],
    );
    let result = parsed(&source, ImportFormat::Docx);
    assert_eq!(result.blocks.len(), 1);
    assert_eq!(result.blocks[0].text, "Trà của Chú Lâm.");
    for code in [
        "docx_tables_flattened",
        "docx_field_instructions_omitted",
        "docx_external_content_omitted",
        "external_relationships_ignored",
        "macros_ignored",
        "embedded_objects_ignored",
        "docx_annotations_omitted",
        "docx_formatting_omitted",
    ] {
        assert!(warnings(&result).contains(&(code, None)), "missing {code}");
    }
}

#[test]
fn docx_rejects_tracked_changes_and_legacy_fonts_in_document_or_styles() {
    for body in [
        "<w:ins><w:p><w:r><w:t>Thay đổi</w:t></w:r></w:p></w:ins>",
        "<w:p><w:r><w:rPr><w:rPrChange/></w:rPr><w:t>Thay đổi</w:t></w:r></w:p>",
    ] {
        failed(
            &docx(&document(body), &[]),
            ImportFormat::Docx,
            "docx_tracked_changes_require_choice",
            None,
        );
    }
    let body =
        r#"<w:p><w:r><w:rPr><w:rFonts w:ascii=".VnTime"/></w:rPr><w:t>Xin chào.</w:t></w:r></w:p>"#;
    failed(
        &docx(&document(body), &[]),
        ImportFormat::Docx,
        "docx_legacy_encoding_requires_choice",
        None,
    );
    let styles = br#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:rFonts w:ascii="VNI-Times"/></w:styles>"#;
    failed(
        &docx(
            &document("<w:p><w:r><w:t>Xin chào.</w:t></w:r></w:p>"),
            &[("word/styles.xml", styles)],
        ),
        ImportFormat::Docx,
        "docx_legacy_encoding_requires_choice",
        None,
    );
}

#[test]
fn docx_rejects_xml_entities_encodings_truncation_and_missing_structure_precisely() {
    let cases = [
        (format!("<!DOCTYPE w:document [<!ENTITY x SYSTEM 'https://example.invalid/'>]>{}", document("<w:p><w:r><w:t>&x;</w:t></w:r></w:p>")), "xml_doctype_forbidden"),
        (document("<w:p><w:r><w:t>&unregistered;</w:t></w:r></w:p>"), "xml_entity_forbidden"),
        (document("<w:p><w:r><w:t>&#0;</w:t></w:r></w:p>"), "xml_entity_forbidden"),
        (document("<w:p><w:r><w:t>Xin chào.</w:t></w:r>"), "malformed_xml"),
        (document("<w:p><w:r><w:t>Xin chào.</w:t></w:r></w:p>").replace("encoding=\"UTF-8\"", "encoding=\"UTF-16\""), "unsupported_xml_encoding"),
        ("<w:document xmlns:w=\"https://example.invalid/namespace\"><w:body/></w:document>".into(), "docx_document_namespace_invalid"),
        ("<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"/>".into(), "docx_body_missing"),
        (document("<w:t>Text outside paragraph</w:t>"), "docx_text_outside_paragraph"),
    ];
    for (xml, code) in cases {
        failed(&docx(&xml, &[]), ImportFormat::Docx, code, None);
    }
    failed(
        &archive(
            &[(
                "word/document.xml",
                document("<w:p><w:r><w:t>x</w:t></w:r></w:p>").as_bytes(),
            )],
            zip::CompressionMethod::Stored,
        ),
        ImportFormat::Docx,
        "docx_main_content_type_missing",
        None,
    );
    failed(
        &archive(
            &[("[Content_Types].xml", CONTENT_TYPES.as_bytes())],
            zip::CompressionMethod::Stored,
        ),
        ImportFormat::Docx,
        "docx_document_missing",
        None,
    );
}

#[test]
fn docx_rejects_unsafe_paths_and_duplicate_directory_names_before_zip_decoding() {
    let xml = document("<w:p><w:r><w:t>x</w:t></w:r></w:p>");
    for path in [
        "../outside.xml",
        "/absolute.xml",
        "word\\evil.xml",
        "C:evil.xml",
        "word/../evil.xml",
        "word//evil.xml",
    ] {
        failed(
            &docx(&xml, &[(path, b"inert")]),
            ImportFormat::Docx,
            "unsafe_archive_path",
            None,
        );
    }
    let mut duplicate = docx(
        &xml,
        &[("word/abcdefgh.xml", b"x"), ("word/ijklmnop.xml", b"x")],
    );
    let positions: Vec<_> = duplicate
        .windows(17)
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == b"word/ijklmnop.xml").then_some(index))
        .collect();
    for position in positions {
        duplicate[position..position + 17].copy_from_slice(b"word/abcdefgh.xml");
    }
    failed(
        &duplicate,
        ImportFormat::Docx,
        "duplicate_archive_entry",
        None,
    );
}

#[test]
fn docx_rejects_unix_symlink_entry_metadata_without_following_its_target() {
    let xml = document("<w:p><w:r><w:t>Xin chào.</w:t></w:r></w:p>");
    let mut source = docx(&xml, &[("word/linked.bin", b"../../private/secret")]);
    let name = b"word/linked.bin";
    let directory_name = source
        .windows(name.len())
        .rposition(|bytes| bytes == name)
        .unwrap();
    let central = directory_name - 46;
    assert_eq!(&source[central..central + 4], b"PK\x01\x02");
    // Construct UNIX symlink metadata independently of the parser. ZipWriter's
    // unix_permissions setter deliberately strips file-type bits.
    source[central + 5] = 3;
    source[central + 38..central + 42].copy_from_slice(&(0o120777_u32 << 16).to_le_bytes());
    failed(
        &source,
        ImportFormat::Docx,
        "archive_symlink_forbidden",
        None,
    );
}

#[test]
fn zip_declared_counts_sizes_encryption_and_expansion_are_bounded_before_allocation() {
    let xml = document("<w:p><w:r><w:t>x</w:t></w:r></w:p>");
    let source = docx(&xml, &[]);
    let end = source
        .windows(4)
        .rposition(|bytes| bytes == b"PK\x05\x06")
        .unwrap();
    let mut count = source.clone();
    count[end + 8..end + 10].copy_from_slice(&257_u16.to_le_bytes());
    count[end + 10..end + 12].copy_from_slice(&257_u16.to_le_bytes());
    failed(&count, ImportFormat::Docx, "archive_entry_limit", None);
    let mut zip64 = source.clone();
    zip64[end + 10..end + 12].copy_from_slice(&u16::MAX.to_le_bytes());
    failed(
        &zip64,
        ImportFormat::Docx,
        "archive_zip64_unsupported",
        None,
    );
    let mut huge_size = source.clone();
    let central = central_offset(&source);
    huge_size[central + 20..central + 24].copy_from_slice(&(u32::MAX - 1).to_le_bytes());
    failed(
        &huge_size,
        ImportFormat::Docx,
        "malformed_archive_entry",
        None,
    );
    let mut encrypted = source.clone();
    encrypted[central + 8..central + 10].copy_from_slice(&1_u16.to_le_bytes());
    failed(
        &encrypted,
        ImportFormat::Docx,
        "malformed_or_encrypted_archive",
        None,
    );
    let mut expansion = source.clone();
    expansion[central + 24..central + 28].copy_from_slice(&(3 * 1024 * 1024_u32).to_le_bytes());
    failed(
        &expansion,
        ImportFormat::Docx,
        "archive_expansion_limit",
        None,
    );
    let bomb = vec![b'x'; 100_000];
    let compressed = archive(
        &[("word/bomb.bin", &bomb)],
        zip::CompressionMethod::Deflated,
    );
    failed(&compressed, ImportFormat::Docx, "archive_ratio_limit", None);
    let names: Vec<_> = (0..MAX_ZIP_ENTRIES - 2)
        .map(|index| format!("word/entry{index}.bin"))
        .collect();
    let entries: Vec<_> = names
        .iter()
        .map(|name| (name.as_str(), b"x".as_slice()))
        .collect();
    assert_eq!(
        parsed(&docx(&xml, &entries), ImportFormat::Docx).blocks[0].text,
        "x"
    );
    failed(
        b"PK\x03\x04truncated",
        ImportFormat::Docx,
        "malformed_archive",
        None,
    );
}

#[test]
fn docx_depth_and_element_count_limits_reject_pathological_xml() {
    let deep = document(&format!(
        "{}<w:p><w:r><w:t>x</w:t></w:r></w:p>{}",
        "<w:div>".repeat(MAX_XML_DEPTH),
        "</w:div>".repeat(MAX_XML_DEPTH)
    ));
    failed(
        &docx(&deep, &[]),
        ImportFormat::Docx,
        "xml_depth_limit",
        None,
    );
    let many = document(&"<w:div/>".repeat(MAX_XML_EVENTS));
    failed(
        &docx(&many, &[]),
        ImportFormat::Docx,
        "xml_event_limit",
        None,
    );
}

#[test]
fn bounded_malformed_byte_smoke_never_panics_and_valid_values_remain_bounded() {
    // Deterministic malformed-input smoke, not a coverage-guided fuzzing claim.
    let mut state = 0x5f37_59df_u32;
    for length in 0..1024 {
        let mut input = Vec::with_capacity(length);
        for _ in 0..length {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            input.push(state as u8);
        }
        for format in [
            ImportFormat::Txt,
            ImportFormat::Markdown,
            ImportFormat::Docx,
            ImportFormat::ScriptIr,
        ] {
            if let ImportOutcome::Parsed { extraction } = extract(&input, format) {
                assert!(extraction.blocks.len() <= MAX_BLOCKS);
                for (index, block) in extraction.blocks.iter().enumerate() {
                    assert_eq!(block.index as usize, index);
                }
            }
        }
    }
    let xml = document("<w:p><w:r><w:t>An: Người dẫn chuyện</w:t></w:r></w:p>");
    let valid = docx(&xml, &[]);
    for length in 4..valid.len() {
        assert!(matches!(
            extract(&valid[..length], ImportFormat::Docx),
            ImportOutcome::Failed { .. }
        ));
    }
}
