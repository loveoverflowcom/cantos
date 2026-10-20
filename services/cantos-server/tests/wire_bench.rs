//! Size and wall-clock comparison of JSON, MessagePack and pilot Protobuf representations on Cantos payloads.
//!
//! A reporting harness, not a statistical benchmark. Sizes (raw and gzip level 6) are exact and
//! deterministic. Times are the median of seven rounds of a fixed iteration count on one
//! machine, printed with the min–max spread of those rounds. Run it deliberately; it is ignored
//! in the normal suite and only the size assertion below runs every time:
//!
//! ```text
//! cargo test -p cantos-server --release --test wire_bench -- --ignored --nocapture
//! ```
//!
//! With `CANTOS_WIRE_PAYLOADS=<dir>` it also writes every payload's JSON and MessagePack bytes,
//! so the same payloads can be decoded and encoded inside a WASM build in a browser.
use std::{fmt::Debug, hint::black_box, io::Write, path::PathBuf, time::Instant};

use cantos_contracts::{
    protobuf::ProtobufDto,
    wire::{decode, decode_pilot, encode, encode_pilot, WireFormat},
    HistoryResponse, ReviewResponse, RevisionResponse, RevisionSummary, SaveRevisionRequest,
    ValidateScriptRequest,
};
use cantos_server::script_ir::read_script;
use flate2::{write::GzEncoder, Compression};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

const FIXTURE: &str =
    include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");

/// Vietnamese lines with the characters JSON must escape and MessagePack carries raw.
const LINES: [&str; 6] = [
    "Ánh đèn cuối cùng còn sáng trên sân khấu.",
    "“Đi thôi!” — anh nói khẽ, rồi im lặng…",
    "Cô hỏi: \"Ngày mai, mình có diễn tiếp không?\"",
    "Đường dẫn C:\\kịch\\cảnh-1 vẫn còn trong sổ tay.",
    "Tiếng mưa rơi tí tách 🎭 trên mái tôn, xa xa có tiếng chuông chùa.",
    "Gió thổi qua hàng tre; Minh đáp: 'Tôi sẽ chờ cậu ở đây.'",
];
const SPEAKERS: [&str; 3] = ["narrator", "an", "minh"];
const EMOTIONS: [&str; 3] = ["calm", "hopeful", "warm"];

/// A complete canonical export with `lines` generated dialogues in one scene.
fn canonical_script(lines: usize) -> String {
    let mut document: Value = serde_json::from_str(FIXTURE).unwrap();
    let act = &mut document["episode"]["acts"][0];
    act["scenes"].as_array_mut().unwrap().truncate(1);
    let dialogues: Vec<Value> = (0..lines)
        .map(|index| {
            // The scene's cue is anchored to dialogue-01, so the first line keeps that ID.
            let id = if index == 0 {
                "dialogue-01".to_owned()
            } else {
                format!("line-{index:05}")
            };
            json!({
                "id": id,
                "speaker_id": SPEAKERS[index % SPEAKERS.len()],
                "text": LINES[index % LINES.len()],
                "delivery": {
                    "emotion": EMOTIONS[index % EMOTIONS.len()],
                    "intensity_permille": (index * 137) % 1001,
                },
                "pronunciation_overrides": [],
            })
        })
        .collect();
    act["scenes"][0]["dialogues"] = Value::Array(dialogues);
    let content = read_script(document.to_string().as_bytes()).unwrap();
    String::from_utf8(content.export_bytes()).unwrap()
}

/// The smallest generated export at or above `target` bytes.
fn script_of_at_least(target: usize) -> String {
    let base = canonical_script(1);
    if base.len() >= target {
        return base;
    }
    let per_line = canonical_script(11).len().saturating_sub(base.len()) / 10;
    let mut lines = 1 + (target - base.len()).div_ceil(per_line.max(1));
    loop {
        let script = canonical_script(lines);
        if script.len() >= target {
            return script;
        }
        lines += 1 + (target - script.len()) / per_line.max(1);
    }
}

fn digest(seed: u64, prefix: &str) -> String {
    format!(
        "{prefix}:sha256:{:064x}",
        seed.wrapping_mul(0x9e37_79b9_7f4a_7c15)
    )
}

fn revision(script_json: &str) -> RevisionResponse {
    RevisionResponse {
        script_id: "ad04abb5-bba0-4240-b211-3ed2aa179999".into(),
        revision: 42,
        accepted_by: "alice".into(),
        accepted_at: "2026-10-11T09:42:11.123456Z".into(),
        content_digest: digest(1, "sir-c1"),
        export_digest: digest(2, "sir-e1"),
        script_json: script_json.into(),
    }
}

fn save(script_json: &str) -> SaveRevisionRequest {
    SaveRevisionRequest {
        expected_revision: 41,
        operation_id: "6f1c2a8e-0b7d-4c3e-9a51-2d4f6b8e0c13".into(),
        script_json: script_json.into(),
    }
}

fn review(revision: u64) -> ReviewResponse {
    ReviewResponse {
        id: format!("2b9d4e6f-1a3c-4e5f-8a7b-{revision:012}"),
        script_id: "ad04abb5-bba0-4240-b211-3ed2aa179999".into(),
        revision,
        operation_id: "6f1c2a8e-0b7d-4c3e-9a51-2d4f6b8e0c13".into(),
        reviewed_by: "alice".into(),
        reviewed_at: "2026-10-11T09:42:11.123456Z".into(),
        content_digest: digest(revision, "sir-c1"),
        export_digest: digest(revision + 1, "sir-e1"),
    }
}

/// One default bounded history page: 20 summaries and their reviews.
fn history() -> HistoryResponse {
    HistoryResponse {
        revisions: (1..=20)
            .map(|revision| RevisionSummary {
                script_id: "ad04abb5-bba0-4240-b211-3ed2aa179999".into(),
                revision,
                accepted_by: if revision % 2 == 0 { "alice" } else { "bob" }.into(),
                accepted_at: format!("2026-10-{:02}T09:42:11.123456Z", revision % 28 + 1),
                content_digest: digest(revision, "sir-c1"),
                export_digest: digest(revision + 7, "sir-e1"),
            })
            .collect(),
        reviews: (1..=20).step_by(4).map(review).collect(),
        next_after: Some(20),
    }
}

fn gzip(bytes: &[u8]) -> usize {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::new(6));
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap().len()
}

struct Case {
    label: String,
    kind: &'static str,
    json: Vec<u8>,
    msgpack: Vec<u8>,
    protobuf: Option<Vec<u8>>,
}

fn case<T: Serialize>(label: impl Into<String>, kind: &'static str, value: &T) -> Case {
    Case {
        label: label.into(),
        kind,
        json: encode(WireFormat::Json, value).unwrap(),
        msgpack: encode(WireFormat::MessagePack, value).unwrap(),
        protobuf: None,
    }
}

fn pilot_case<T: Serialize + ProtobufDto>(
    label: impl Into<String>,
    kind: &'static str,
    value: &T,
) -> Case {
    let mut case = case(label, kind, value);
    case.protobuf = Some(encode_pilot(WireFormat::Protobuf, value).unwrap());
    case
}

const SCRIPT_TARGETS: [(&str, usize); 4] = [
    ("1 KiB", 1024),
    ("8 KiB", 8 * 1024),
    ("64 KiB", 64 * 1024),
    ("512 KiB", 512 * 1024),
];

fn cases() -> Vec<Case> {
    let mut cases = vec![
        case("review metadata", "review", &review(42)),
        case("history page, 20 revisions", "history", &history()),
    ];
    for (label, target) in SCRIPT_TARGETS {
        let script = script_of_at_least(target);
        cases.push(pilot_case(
            format!("Revision, IR {label}"),
            "revision",
            &revision(&script),
        ));
        cases.push(pilot_case(
            format!("SaveRevision, IR {label}"),
            "save",
            &save(&script),
        ));
        cases.push(pilot_case(
            format!("ValidateScript, IR {label}"),
            "validate",
            &ValidateScriptRequest {
                script_json: script,
            },
        ));
    }
    cases
}

#[test]
fn msgpack_is_never_larger_than_json_and_strictly_smaller_once_script_text_is_carried() {
    for case in cases() {
        assert!(
            case.msgpack.len() <= case.json.len(),
            "{}: msgpack {} B > json {} B",
            case.label,
            case.msgpack.len(),
            case.json.len()
        );
        if case.kind != "review" && case.kind != "history" {
            // Script IR text is JSON inside a JSON string: every quote and backslash is escaped
            // again in JSON and carried raw in MessagePack.
            assert!(case.msgpack.len() < case.json.len(), "{}", case.label);
        }
    }
}

#[test]
fn protobuf_size_and_roundtrip_are_measured_only_for_schema_pilot_dtos() {
    for case in cases().into_iter().filter(|case| case.protobuf.is_some()) {
        assert!(
            case.protobuf.as_ref().unwrap().len() < case.json.len(),
            "{}",
            case.label
        );
        match case.kind {
            "revision" => {
                assert_protobuf_roundtrip::<RevisionResponse>(&case);
            }
            "save" => {
                assert_protobuf_roundtrip::<SaveRevisionRequest>(&case);
            }
            "validate" => {
                assert_protobuf_roundtrip::<ValidateScriptRequest>(&case);
            }
            other => panic!("unexpected pilot {other}"),
        }
    }
}

/// Median and spread of nanoseconds per operation over `rounds`.
fn measure(rounds: usize, iterations: usize, mut operation: impl FnMut()) -> (f64, f64, f64) {
    let mut samples: Vec<f64> = (0..rounds)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..iterations {
                operation();
            }
            start.elapsed().as_nanos() as f64 / iterations as f64
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    (samples[rounds / 2], samples[0], samples[rounds - 1])
}

fn timings<T>(case: &Case) -> [(f64, f64, f64); 4]
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let value: T = decode(WireFormat::Json, &case.json).unwrap();
    assert_eq!(
        decode::<T>(WireFormat::MessagePack, &case.msgpack).unwrap(),
        value
    );
    let iterations = (4_000_000 / case.json.len()).clamp(20, 50_000);
    let rounds = 7;
    [
        measure(rounds, iterations, || {
            black_box(encode(WireFormat::Json, black_box(&value)).unwrap());
        }),
        measure(rounds, iterations, || {
            black_box(encode(WireFormat::MessagePack, black_box(&value)).unwrap());
        }),
        measure(rounds, iterations, || {
            black_box(decode::<T>(WireFormat::Json, black_box(&case.json)).unwrap());
        }),
        measure(rounds, iterations, || {
            black_box(decode::<T>(WireFormat::MessagePack, black_box(&case.msgpack)).unwrap());
        }),
    ]
}

fn assert_protobuf_roundtrip<T>(case: &Case) -> T
where
    T: Serialize + DeserializeOwned + ProtobufDto + PartialEq + Debug,
{
    let value: T = decode(WireFormat::Json, &case.json).unwrap();
    let bytes = case.protobuf.as_ref().unwrap();
    assert_eq!(
        decode_pilot::<T>(WireFormat::Protobuf, bytes).unwrap(),
        value
    );
    value
}

fn protobuf_timings<T>(case: &Case) -> [(f64, f64, f64); 2]
where
    T: Serialize + DeserializeOwned + ProtobufDto + PartialEq + Debug,
{
    let value = assert_protobuf_roundtrip::<T>(case);
    let bytes = case.protobuf.as_ref().unwrap();
    let iterations = (4_000_000 / case.json.len()).clamp(20, 50_000);
    [
        measure(7, iterations, || {
            black_box(encode_pilot(WireFormat::Protobuf, black_box(&value)).unwrap());
        }),
        measure(7, iterations, || {
            black_box(decode_pilot::<T>(WireFormat::Protobuf, black_box(bytes)).unwrap());
        }),
    ]
}

fn micros((median, low, high): (f64, f64, f64)) -> String {
    format!(
        "{:.2} ({:.2}–{:.2})",
        median / 1000.0,
        low / 1000.0,
        high / 1000.0
    )
}

#[test]
#[ignore = "reporting harness; run explicitly with --release -- --ignored --nocapture"]
fn report_wire_codec_costs() {
    let output = std::env::var_os("CANTOS_WIRE_PAYLOADS").map(PathBuf::from);
    println!("\n| Payload | JSON B | MessagePack B | ratio | JSON gzip B | MessagePack gzip B | gzip ratio |");
    println!("|---|---:|---:|---:|---:|---:|---:|");
    let cases = cases();
    for case in &cases {
        let (json_gzip, msgpack_gzip) = (gzip(&case.json), gzip(&case.msgpack));
        println!(
            "| {} | {} | {} | {:.3} | {} | {} | {:.3} |",
            case.label,
            case.json.len(),
            case.msgpack.len(),
            case.msgpack.len() as f64 / case.json.len() as f64,
            json_gzip,
            msgpack_gzip,
            msgpack_gzip as f64 / json_gzip as f64
        );
        if let Some(directory) = &output {
            std::fs::create_dir_all(directory).unwrap();
            let stem = case.label.replace([' ', ',', '/'], "_");
            std::fs::write(
                directory.join(format!("{}.{stem}.json", case.kind)),
                &case.json,
            )
            .unwrap();
            std::fs::write(
                directory.join(format!("{}.{stem}.msgpack", case.kind)),
                &case.msgpack,
            )
            .unwrap();
        }
    }
    println!("\nMicroseconds per operation: median of 7 rounds (min–max of the rounds).\n");
    println!("| Payload | JSON encode | MessagePack encode | JSON decode | MessagePack decode |");
    println!("|---|---:|---:|---:|---:|");
    for case in &cases {
        let [json_encode, msgpack_encode, json_decode, msgpack_decode] = match case.kind {
            "review" => timings::<ReviewResponse>(case),
            "history" => timings::<HistoryResponse>(case),
            "revision" => timings::<RevisionResponse>(case),
            "save" => timings::<SaveRevisionRequest>(case),
            "validate" => timings::<ValidateScriptRequest>(case),
            other => unreachable!("{other}"),
        };
        println!(
            "| {} | {} | {} | {} | {} |",
            case.label,
            micros(json_encode),
            micros(msgpack_encode),
            micros(json_decode),
            micros(msgpack_decode)
        );
    }
}

#[test]
#[ignore = "reporting harness; run explicitly with --release -- --ignored --nocapture"]
fn report_protobuf_pilot_costs_against_json_and_legacy_msgpack() {
    let output = std::env::var_os("CANTOS_WIRE_PAYLOADS").map(PathBuf::from);
    println!("Native one-machine reporting only; no heap/allocation, browser, CMP or request-latency claim.");
    println!("Payload | JSON B | MessagePack B | Protobuf B | JSON gzip B | MessagePack gzip B | Protobuf gzip B");
    let cases: Vec<_> = cases()
        .into_iter()
        .filter(|case| case.protobuf.is_some())
        .collect();
    for case in &cases {
        let protobuf = case.protobuf.as_ref().unwrap();
        println!(
            "{} | {} | {} | {} | {} | {} | {}",
            case.label,
            case.json.len(),
            case.msgpack.len(),
            protobuf.len(),
            gzip(&case.json),
            gzip(&case.msgpack),
            gzip(protobuf)
        );
        if let Some(directory) = &output {
            std::fs::create_dir_all(directory).unwrap();
            let stem = case.label.replace([' ', ',', '/'], "_");
            for (extension, bytes) in [
                ("json", &case.json),
                ("msgpack", &case.msgpack),
                ("protobuf", protobuf),
            ] {
                std::fs::write(
                    directory.join(format!("{}.{stem}.{extension}", case.kind)),
                    bytes,
                )
                .unwrap();
            }
        }
    }
    println!("Microseconds/op, median of 7 rounds (min-max), JSON encode | MessagePack encode | Protobuf encode | JSON decode | MessagePack decode | Protobuf decode");
    for case in &cases {
        let (baseline, protobuf) = match case.kind {
            "revision" => (
                timings::<RevisionResponse>(case),
                protobuf_timings::<RevisionResponse>(case),
            ),
            "save" => (
                timings::<SaveRevisionRequest>(case),
                protobuf_timings::<SaveRevisionRequest>(case),
            ),
            "validate" => (
                timings::<ValidateScriptRequest>(case),
                protobuf_timings::<ValidateScriptRequest>(case),
            ),
            other => unreachable!("{other}"),
        };
        let [je, me, jd, md] = baseline;
        let [pe, pd] = protobuf;
        println!(
            "{} | {} | {} | {} | {} | {} | {}",
            case.label,
            micros(je),
            micros(me),
            micros(pe),
            micros(jd),
            micros(md),
            micros(pd)
        );
    }
}
