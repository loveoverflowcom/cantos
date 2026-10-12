//! Fresh evidence about a registered clip's stored object, gathered outside any transaction.
//!
//! The only object store that exists is the worker's private local output root, opened with the
//! same hardened reader recovery uses: it rejects links, devices, oversize files, a different
//! attempt directory and anything outside the configured root. The bytes are then inspected under
//! the clip's own output contract and hashed. No object storage, CDN or signed URL is involved or
//! implied; a clip whose root this process cannot open is simply `Missing`.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::contexts::production::catalog::local as local_contract;
use crate::contexts::synthesis::{
    fingerprint::SpeechFingerprint,
    media::audio::{inspect_output_for_profile, OutputProfile},
    reuse::{ArtifactProbe, AudioDigest, ClipCandidate, StoredClip},
    worker::{prepare_root, read_output, WorkerError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsafeRoot;

impl std::fmt::Display for UnsafeRoot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("output root must be an existing private directory")
    }
}

impl std::error::Error for UnsafeRoot {}

/// The profile a stored object must satisfy. Unknown contracts have no inspector, so they never
/// verify: a clip is not trusted merely because nothing can check it.
fn profile_for(output_contract: &str) -> Option<OutputProfile> {
    (output_contract == local_contract::OUTPUT_CONTRACT).then_some(OutputProfile::Espeak22Khz)
}

/// Proof that one registered clip's object was just read safely, inspected under its own contract
/// and found to hash to exactly the registered checksum. Fields are private and there is no
/// `Deserialize` or public constructor: only [`ClipProber::inspect`] can make one, so a selection
/// that requires it cannot be handed a digest nobody observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedObject {
    clip_id: String,
    audio_sha256: AudioDigest,
}

impl VerifiedObject {
    pub fn clip_id(&self) -> &str {
        &self.clip_id
    }

    pub fn audio_sha256(&self) -> &AudioDigest {
        &self.audio_sha256
    }
}

/// The observed state of one stored object and, only when it fully verified, the witness.
#[derive(Debug, Clone)]
pub struct Inspection {
    probe: ArtifactProbe,
    verified: Option<VerifiedObject>,
}

impl Inspection {
    pub fn probe(&self) -> &ArtifactProbe {
        &self.probe
    }

    pub fn into_parts(self) -> (ArtifactProbe, Option<VerifiedObject>) {
        (self.probe, self.verified)
    }
}

#[derive(Debug, Clone)]
pub struct ClipProber {
    root: PathBuf,
}

impl ClipProber {
    /// The root must be an existing private directory (no group/other access, not a link).
    pub fn open(root: &Path) -> Result<Self, UnsafeRoot> {
        prepare_root(root)
            .map(|root| Self { root })
            .map_err(|_| UnsafeRoot)
    }

    /// Probe a registered clip's object and, if its bytes hash to the registered checksum, issue the
    /// witness a selection requires. Blocking file I/O: call from a blocking task.
    pub fn inspect(&self, stored: &StoredClip) -> Inspection {
        let probe = self.probe(
            &stored.attempt_id,
            &stored.output_path,
            &stored.clip.output_contract,
        );
        let verified = match &probe {
            ArtifactProbe::Present { audio_sha256 }
                if audio_sha256 == &stored.clip.audio_sha256 =>
            {
                Some(VerifiedObject {
                    clip_id: stored.clip.id.clone(),
                    audio_sha256: audio_sha256.clone(),
                })
            }
            _ => None,
        };
        Inspection { probe, verified }
    }

    /// Read, inspect and hash the stored object. Blocking file I/O: call from a blocking task.
    pub fn probe(
        &self,
        attempt_id: &str,
        output_path: &str,
        output_contract: &str,
    ) -> ArtifactProbe {
        let Some(profile) = profile_for(output_contract) else {
            return ArtifactProbe::Invalid;
        };
        let bytes = match read_output(&self.root, output_path, attempt_id) {
            Ok(bytes) => bytes,
            // Missing bytes, denied access and I/O faults are unavailable, not corruption.
            Err(WorkerError::OutputIo) => return ArtifactProbe::Missing,
            Err(_) => return ArtifactProbe::Invalid,
        };
        let digest = AudioDigest::of_bytes(&bytes);
        match inspect_output_for_profile(&bytes, digest.as_str(), profile) {
            Ok(_) => ArtifactProbe::Present {
                audio_sha256: digest,
            },
            Err(_) => ArtifactProbe::Invalid,
        }
    }
}

/// Probe every stored clip once. Without a prober nothing is verified, so every candidate is
/// `NotProbed` and the plan counts none of them as a hit. Blocking file I/O.
pub fn probe_candidates(
    prober: Option<&ClipProber>,
    clips: &BTreeMap<SpeechFingerprint, Vec<StoredClip>>,
) -> BTreeMap<SpeechFingerprint, Vec<ClipCandidate>> {
    clips
        .iter()
        .map(|(fingerprint, stored)| {
            let candidates = stored
                .iter()
                .map(|stored| ClipCandidate {
                    clip: stored.clip.clone(),
                    probe: prober.map_or(ArtifactProbe::NotProbed, |prober| {
                        prober.inspect(stored).probe
                    }),
                })
                .collect();
            (fingerprint.clone(), candidates)
        })
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        os::unix::fs::{symlink, OpenOptionsExt, PermissionsExt},
    };

    use super::*;
    use crate::contexts::synthesis::{
        fingerprint::SpeechInput,
        jobs::ExecutionMode,
        reuse::{RegisteredClip, StoredClip},
    };

    const ATTEMPT: &str = "5b7b7e4e-0a1d-4a0e-9d44-3f3f2d6d1a01";
    const OTHER_ATTEMPT: &str = "9d2f5d0a-6b1c-4c0e-8f60-0d3a7f1b2c02";

    fn wav(sample_rate: u32, seed: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&40_u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(&[seed, 0, 1, 0]);
        bytes
    }

    struct Root(PathBuf);

    impl Root {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("cantos-probe-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }

        fn directory(&self, attempt: &str) -> PathBuf {
            let directory = self.0.join(attempt);
            fs::create_dir_all(&directory).unwrap();
            directory
        }

        fn place(&self, attempt: &str, bytes: &[u8]) -> String {
            let path = self.directory(attempt).join("output.wav");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .unwrap();
            file.write_all(bytes).unwrap();
            path.to_str().unwrap().to_owned()
        }

        fn prober(&self) -> ClipProber {
            ClipProber::open(&self.0).unwrap()
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const CONTRACT: &str = local_contract::OUTPUT_CONTRACT;

    #[test]
    fn a_valid_object_is_present_with_the_digest_of_its_exact_bytes() {
        let root = Root::new();
        let bytes = wav(22_050, 7);
        let path = root.place(ATTEMPT, &bytes);
        assert_eq!(
            root.prober().probe(ATTEMPT, &path, CONTRACT),
            ArtifactProbe::Present {
                audio_sha256: AudioDigest::of_bytes(&bytes)
            }
        );
    }

    #[test]
    fn absent_objects_are_missing_not_invalid() {
        let root = Root::new();
        let path = root.0.join(ATTEMPT).join("output.wav");
        let path = path.to_str().unwrap();
        // No attempt directory at all, then an empty one.
        assert_eq!(
            root.prober().probe(ATTEMPT, path, CONTRACT),
            ArtifactProbe::Missing
        );
        root.directory(ATTEMPT);
        assert_eq!(
            root.prober().probe(ATTEMPT, path, CONTRACT),
            ArtifactProbe::Missing
        );
    }

    #[test]
    fn bytes_that_are_not_audio_for_the_contract_are_invalid() {
        let root = Root::new();
        for (index, bytes) in [
            Vec::new(),
            b"not a wav file".to_vec(),
            wav(22_050, 1)[..30].to_vec(),
            // Well formed, but another profile's sample rate.
            wav(24_000, 1),
        ]
        .into_iter()
        .enumerate()
        {
            let attempt = format!("00000000-0000-4000-8000-00000000000{index}");
            let path = root.place(&attempt, &bytes);
            assert_eq!(
                root.prober().probe(&attempt, &path, CONTRACT),
                ArtifactProbe::Invalid,
                "case {index}"
            );
        }
    }

    #[test]
    fn links_directories_and_foreign_paths_are_invalid_never_followed() {
        let root = Root::new();
        let bytes = wav(22_050, 3);
        let original = root.0.join("original.wav");
        fs::write(&original, &bytes).unwrap();
        fs::set_permissions(&original, fs::Permissions::from_mode(0o600)).unwrap();

        let directory = root.directory(ATTEMPT);
        let output = directory.join("output.wav");
        let path = output.to_str().unwrap();
        symlink(&original, &output).unwrap();
        assert_eq!(
            root.prober().probe(ATTEMPT, path, CONTRACT),
            ArtifactProbe::Invalid
        );
        fs::remove_file(&output).unwrap();
        fs::hard_link(&original, &output).unwrap();
        assert_eq!(
            root.prober().probe(ATTEMPT, path, CONTRACT),
            ArtifactProbe::Invalid
        );
        fs::remove_file(&output).unwrap();
        fs::create_dir(&output).unwrap();
        assert_eq!(
            root.prober().probe(ATTEMPT, path, CONTRACT),
            ArtifactProbe::Invalid
        );
        fs::remove_dir(&output).unwrap();

        // A valid file is still refused when asked for under another attempt or outside the root.
        let good = root.place(OTHER_ATTEMPT, &bytes);
        assert!(matches!(
            root.prober().probe(OTHER_ATTEMPT, &good, CONTRACT),
            ArtifactProbe::Present { .. }
        ));
        assert_eq!(
            root.prober().probe(ATTEMPT, &good, CONTRACT),
            ArtifactProbe::Invalid
        );
        let elsewhere = Root::new();
        assert_eq!(
            elsewhere.prober().probe(OTHER_ATTEMPT, &good, CONTRACT),
            ArtifactProbe::Invalid
        );
    }

    #[test]
    fn a_contract_without_an_inspector_never_verifies() {
        let root = Root::new();
        let path = root.place(ATTEMPT, &wav(22_050, 5));
        for contract in [
            "wav-pcm-s16le-mono-24000-v1",
            "wav-pcm-s16le-mono-24000-vieneu-v1",
            "",
            "wav-pcm-s16le-mono-22050-v2",
        ] {
            assert_eq!(
                root.prober().probe(ATTEMPT, &path, contract),
                ArtifactProbe::Invalid,
                "{contract:?}"
            );
        }
    }

    #[test]
    fn only_an_existing_private_directory_can_be_an_output_root() {
        let root = Root::new();
        fs::set_permissions(&root.0, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(ClipProber::open(&root.0).err(), Some(UnsafeRoot));
        fs::set_permissions(&root.0, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(ClipProber::open(&root.0).is_ok());
        assert_eq!(
            ClipProber::open(&root.0.join("does-not-exist")).err(),
            Some(UnsafeRoot)
        );
        let link = root.0.join("link");
        symlink(&root.0, &link).unwrap();
        assert_eq!(ClipProber::open(&link).err(), Some(UnsafeRoot));
    }

    fn stored(attempt: &str, path: &str, bytes: &[u8], sequence: i64) -> StoredClip {
        let input = cantos_contracts::ResolvedSynthesisInput {
            dialogue_id: "dialogue-01".into(),
            character_id: "an".into(),
            text: "Xin chào.".into(),
            language: "vi".into(),
            provider_id: local_contract::PROVIDER_ID.into(),
            model_id: local_contract::MODEL_ID.into(),
            voice_id: "vi".into(),
            voice_rights_record_id: "voice-espeak".into(),
            adapter_version: local_contract::ADAPTER_VERSION.into(),
            output_contract: CONTRACT.into(),
            performance: cantos_contracts::SynthesisPerformance {
                rate_permille: 1_000,
                pitch_semitones: 0,
                emotion: Some("neutral".into()),
                intensity_permille: Some(0),
            },
            pronunciation: Vec::new(),
        };
        StoredClip {
            clip: RegisteredClip {
                id: format!("clip-{sequence}"),
                sequence,
                fingerprint: SpeechInput::from_resolved(&input).unwrap().fingerprint(),
                owner_id: "alice".into(),
                producer: ExecutionMode::LocalEspeak,
                voice_rights_record_id: "voice-espeak".into(),
                output_contract: CONTRACT.into(),
                audio_sha256: AudioDigest::of_bytes(bytes),
            },
            attempt_id: attempt.into(),
            output_path: path.into(),
        }
    }

    #[test]
    fn candidates_are_probed_individually_and_never_without_a_prober() {
        let root = Root::new();
        let good_bytes = wav(22_050, 9);
        let good = root.place(ATTEMPT, &good_bytes);
        let gone = root.0.join(OTHER_ATTEMPT).join("output.wav");
        let clips = [
            stored(ATTEMPT, &good, &good_bytes, 1),
            stored(OTHER_ATTEMPT, gone.to_str().unwrap(), &good_bytes, 2),
        ];
        let fingerprint = clips[0].clip.fingerprint.clone();
        let by_fingerprint: BTreeMap<_, _> = [(fingerprint.clone(), clips.to_vec())].into();

        let probed = probe_candidates(Some(&root.prober()), &by_fingerprint);
        assert_eq!(
            probed[&fingerprint]
                .iter()
                .map(|candidate| candidate.probe.clone())
                .collect::<Vec<_>>(),
            [
                ArtifactProbe::Present {
                    audio_sha256: AudioDigest::of_bytes(&good_bytes)
                },
                ArtifactProbe::Missing
            ]
        );
        let unprobed = probe_candidates(None, &by_fingerprint);
        assert!(unprobed[&fingerprint]
            .iter()
            .all(|candidate| candidate.probe == ArtifactProbe::NotProbed));
    }

    #[test]
    fn a_witness_exists_only_for_an_object_that_hashes_to_its_registered_checksum() {
        let root = Root::new();
        let bytes = wav(22_050, 9);
        let path = root.place(ATTEMPT, &bytes);
        let good = stored(ATTEMPT, &path, &bytes, 1);
        let inspection = root.prober().inspect(&good);
        let (probe, witness) = inspection.into_parts();
        assert!(matches!(probe, ArtifactProbe::Present { .. }));
        let witness = witness.expect("a verified object yields a witness");
        assert_eq!(witness.clip_id(), "clip-1");
        assert_eq!(witness.audio_sha256(), &AudioDigest::of_bytes(&bytes));

        // Valid audio whose checksum is not the registered one: present, but no witness.
        let swapped = stored(ATTEMPT, &path, &wav(22_050, 10), 2);
        let (probe, witness) = root.prober().inspect(&swapped).into_parts();
        assert!(matches!(probe, ArtifactProbe::Present { .. }));
        assert!(witness.is_none());

        // Missing and unsafe objects never yield one.
        let gone = stored(OTHER_ATTEMPT, "/nowhere/output.wav", &bytes, 3);
        assert!(root.prober().inspect(&gone).into_parts().1.is_none());
    }
}
