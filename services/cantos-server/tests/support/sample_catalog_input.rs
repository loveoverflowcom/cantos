//! Issue #427's external input. The proposed JSON is retained verbatim, never mistaken for
//! the Server wire contract; image identity is pinned to the supplied package's checksums.
use std::{collections::BTreeSet, fs, path::Path};

use cantos_server::contexts::media::{domain::image::ImageMime, image_intake::ImageAdmission};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const DATASET: &str = "cantos-sample-catalog-v1";
pub const COVERS: [(&str, &str, &str, usize); 6] = [
    (
        "ben-suong",
        "Bến Sương",
        "8a8dc4785a5f6a8f2421a7f1286595488d6f191cc24c9fc7af47bf1652c60294",
        2_865_230,
    ),
    (
        "nguoi-giu-den",
        "Người Giữ Đèn",
        "9b2b3c4007c778ca5d968c28738f72863ace367028d843c615badbb6bcdec98c",
        3_199_645,
    ),
    (
        "thanh-pho-ngu-quen",
        "Thành Phố Ngủ Quên",
        "e2e916ea7d419d3ded35fc12d9a3a217651d135e06954e1c61861ce24833469b",
        2_856_926,
    ),
    (
        "khu-vuon-tren-may",
        "Khu Vườn Trên Mây",
        "a9bbe79e9bb5c1c3aaab4beb72aaad1a9a00d77e7d4f504d6b29a2d9d020217c",
        2_552_415,
    ),
    (
        "chuyen-tau-cuoi-cung",
        "Chuyến Tàu Cuối Cùng",
        "6403ae919efba3b63fbef9829d9314ff1ba3005392718f7c2989200d36d0c461",
        2_179_204,
    ),
    (
        "mua-gio-qua-nha",
        "Mùa Gió Qua Nhà",
        "b6e65ff3f6b60dbfa75001f4e12c8f09a23b12d992779ae28429561ed055ceb9",
        2_403_571,
    ),
];

#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    Json,
    Dataset,
    Counts,
    Mapping,
    AudioClaim,
    SourceMissing(String),
    SourceIdentity(String),
    ImageRejected(&'static str),
}

#[derive(Deserialize)]
pub struct Input {
    pub dataset_id: String,
    story_count: usize,
    chapter_count: usize,
    pub works: Vec<Work>,
}

#[derive(Deserialize)]
pub struct Work {
    pub sample_key: String,
    pub title: String,
    pub kind: String,
    pub description: String,
    pub language: String,
    pub author: String,
    pub narrator: String,
    pub genre: String,
    pub tags: Vec<String>,
    pub cover: Cover,
    pub chapters: Vec<Chapter>,
}

#[derive(Deserialize)]
pub struct Cover {
    pub relative_path: String,
    pub source_drive_file_id: String,
    pub alt: String,
}

#[derive(Deserialize)]
pub struct Chapter {
    pub sample_key: String,
    pub position: usize,
    pub title: String,
    pub description: String,
    pub audio: Value,
}

pub struct CoverBytes {
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn identity(key: &str, purpose: &str) -> String {
    let digest = Sha256::digest(format!("{DATASET}\0{key}\0{purpose}"));
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

pub fn parse(bytes: &[u8]) -> Result<Input, InputError> {
    let input: Input = serde_json::from_slice(bytes).map_err(|_| InputError::Json)?;
    if input.dataset_id != DATASET {
        return Err(InputError::Dataset);
    }
    if input.story_count != 6 || input.chapter_count != 12 || input.works.len() != 6 {
        return Err(InputError::Counts);
    }
    let mut keys = BTreeSet::new();
    for work in &input.works {
        let expected = COVERS.iter().find(|cover| cover.0 == work.sample_key);
        if expected.is_none_or(|cover| cover.1 != work.title)
            || !keys.insert(&work.sample_key)
            || work.cover.relative_path != format!("covers/{}.avif", work.sample_key)
            || !matches!(work.kind.as_str(), "novel" | "story")
            || work.language != "vi-VN"
            || work.author.trim().is_empty()
            || work.narrator.trim().is_empty()
            || work.genre.trim().is_empty()
            || work.tags.is_empty()
            || work.chapters.len() != 2
        {
            return Err(InputError::Mapping);
        }
        for (position, chapter) in work.chapters.iter().enumerate() {
            if chapter.position != position
                || chapter.sample_key != format!("{}/chapter-{:02}", work.sample_key, position + 1)
            {
                return Err(InputError::Mapping);
            }
            let expected_url = format!(
                "https://media.example.invalid/cantos-samples/{}/chapter-{:02}.mp3",
                work.sample_key,
                position + 1
            );
            if chapter.audio["mock_url"] != expected_url
                || chapter.audio["is_mock"] != true
                || chapter.audio["file_exists"] != false
                || chapter.audio["playback_verified"] != false
                || chapter.audio["mime_type"] != "audio/mpeg"
                || ["duration_ms", "sha256", "byte_length"]
                    .iter()
                    .any(|key| chapter.audio.get(key) != Some(&Value::Null))
            {
                return Err(InputError::AudioClaim);
            }
        }
    }
    Ok(input)
}

pub async fn decode(bytes: Vec<u8>) -> Result<(u32, u32), InputError> {
    let image = ImageAdmission::try_acquire()
        .map_err(|error| InputError::ImageRejected(error.code()))?
        .inspect(bytes, ImageMime::Avif)
        .await
        .map_err(|error| InputError::ImageRejected(error.code()))?;
    Ok((image.metadata().width_px(), image.metadata().height_px()))
}

pub async fn load_covers(input: &Input, source: &Path) -> Result<Vec<CoverBytes>, InputError> {
    let mut covers = Vec::new();
    for work in &input.works {
        let path = source.join(&work.cover.relative_path);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| InputError::SourceMissing(work.cover.relative_path.clone()))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(InputError::SourceIdentity(work.cover.relative_path.clone()));
        }
        let expected = COVERS
            .iter()
            .find(|cover| cover.0 == work.sample_key)
            .ok_or(InputError::Mapping)?;
        if metadata.len() != expected.3 as u64 {
            return Err(InputError::SourceIdentity(work.cover.relative_path.clone()));
        }
        let bytes = fs::read(&path)
            .map_err(|_| InputError::SourceMissing(work.cover.relative_path.clone()))?;
        let sha256 = hash(&bytes);
        if sha256 != expected.2 {
            return Err(InputError::SourceIdentity(work.cover.relative_path.clone()));
        }
        let (width, height) = decode(bytes.clone()).await?;
        covers.push(CoverBytes {
            bytes,
            sha256,
            width,
            height,
        });
    }
    Ok(covers)
}
