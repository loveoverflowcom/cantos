//! Differential oracle: the intake contract's own table against Forge's profile values.

use cantos_forge::domain::audio_profile::{AudioProfile, MAX_BODY_BYTES, MAX_DURATION_SECONDS};

fn contract() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../services/cantos-server/contracts/studio/audio-intake/v1/README.md"
    ))
    .expect("the intake contract is part of the repository")
}

fn bits_per_second(cell: &str) -> u64 {
    cell.trim_end_matches(" bits/s")
        .replace(',', "")
        .parse()
        .unwrap_or_else(|_| panic!("a bits/s number, found {cell:?}"))
}

#[test]
fn profiles_match_the_intake_contract_table() {
    let mut rows = 0;
    for line in contract().lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let [_, id, nominal, maximum, _] = cells.as_slice() else {
            continue;
        };
        let Some(id) = id.strip_prefix('`').and_then(|id| id.strip_suffix('`')) else {
            continue;
        };
        let Ok(profile) = AudioProfile::parse(id) else {
            continue;
        };
        assert_eq!(
            bits_per_second(nominal),
            profile.nominal_bitrate_bps(),
            "{id}"
        );
        assert_eq!(
            bits_per_second(maximum),
            profile.maximum_bitrate_bps(),
            "{id}"
        );
        rows += 1;
    }
    assert_eq!(
        rows,
        AudioProfile::ALL.len(),
        "every profile has a contract row"
    );
}

#[test]
fn every_contract_profile_row_is_known_to_forge() {
    let text = contract();
    let known: Vec<&str> = text
        .lines()
        .filter_map(|line| {
            let first = line.split('|').nth(1)?.trim();
            let id = first.strip_prefix('`')?.strip_suffix('`')?;
            id.starts_with("opus-mono-").then_some(id)
        })
        .collect();
    assert_eq!(known.len(), AudioProfile::ALL.len());
    for id in known {
        assert!(
            AudioProfile::parse(id).is_ok(),
            "{id} is not a Forge profile"
        );
    }
}

#[test]
fn the_size_and_duration_limits_appear_in_the_contract() {
    let text = contract().replace('\n', " ");
    assert!(text.contains("3,600 seconds"));
    assert!(text.contains("33,554,432 bytes (32 MiB)"));
    assert_eq!(MAX_DURATION_SECONDS, 3_600);
    assert_eq!(MAX_BODY_BYTES, 33_554_432);
}
