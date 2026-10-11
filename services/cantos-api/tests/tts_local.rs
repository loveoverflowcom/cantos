//! Synthetic process fixtures only. Live installed-engine evidence is separate.

use cantos_api::tts::local::{
    run_local, LocalPhase, LocalRequestError, LocalRunError, LocalSynthesisRequest,
    ADAPTER_VERSION, MAX_RUNTIME_SECONDS, MAX_TEXT_BYTES, MODEL_ID, OUTPUT_CONTRACT, PROVIDER_ID,
};
use cantos_contracts::{ResolvedSynthesisInput, SynthesisPerformance};
use std::time::Duration;

fn resolved() -> ResolvedSynthesisInput {
    ResolvedSynthesisInput {
        dialogue_id: "dialogue-1".into(),
        character_id: "narrator".into(),
        text: "Ánh đèn còn sáng, đây là mơ".into(),
        language: "vi-VN".into(),
        provider_id: PROVIDER_ID.into(),
        model_id: MODEL_ID.into(),
        voice_id: "vi".into(),
        voice_rights_record_id: "synthetic-voice-rights".into(),
        adapter_version: ADAPTER_VERSION.into(),
        output_contract: OUTPUT_CONTRACT.into(),
        performance: SynthesisPerformance {
            rate_permille: 1_000,
            pitch_semitones: 0,
            emotion: Some("neutral".into()),
            intensity_permille: Some(0),
        },
        pronunciation: vec![],
    }
}

#[test]
fn only_the_exact_local_identity_and_supported_inputs_are_admitted() {
    for voice in ["vi", "vi-vn-x-central", "vi-vn-x-south"] {
        for language in ["vi", "vi-VN"] {
            let mut input = resolved();
            input.voice_id = voice.into();
            input.language = language.into();
            let request = LocalSynthesisRequest::from_resolved(&input).unwrap();
            assert_eq!(request.resolved_input(), &input);
            assert_eq!(request.voice_id(), voice);
            assert_eq!(request.dialogue_id(), "dialogue-1");
            assert_eq!(request.text(), input.text);
            assert!(!format!("{request:?}").contains(&input.text));
        }
    }
}

#[test]
fn mismatched_provider_model_adapter_and_format_have_exact_errors() {
    let cases = [
        ("provider", LocalRequestError::WrongProvider),
        ("model", LocalRequestError::WrongModel),
        ("adapter", LocalRequestError::WrongAdapter),
        ("format", LocalRequestError::WrongOutputContract),
        ("voice", LocalRequestError::UnsupportedVoice),
        ("language", LocalRequestError::UnsupportedLanguage),
    ];
    for (field, expected) in cases {
        let mut input = resolved();
        match field {
            "provider" => input.provider_id = "reference-only".into(),
            "model" => input.model_id = "espeak-ng-1.51.0".into(),
            "adapter" => input.adapter_version = "local-v0".into(),
            "format" => input.output_contract = "reference-only/no-audio".into(),
            "voice" => input.voice_id = "en".into(),
            "language" => input.language = "en".into(),
            _ => unreachable!(),
        }
        assert_eq!(LocalSynthesisRequest::from_resolved(&input), Err(expected));
    }
}

#[test]
fn controls_are_rejected_instead_of_silently_coerced() {
    let mut input = resolved();
    input.performance.rate_permille = 999;
    assert_eq!(
        LocalSynthesisRequest::from_resolved(&input),
        Err(LocalRequestError::UnsupportedRate)
    );
    let mut input = resolved();
    input.performance.pitch_semitones = 1;
    assert_eq!(
        LocalSynthesisRequest::from_resolved(&input),
        Err(LocalRequestError::UnsupportedPitch)
    );
    for emotion in [None, Some("calm".into()), Some("NEUTRAL".into())] {
        let mut input = resolved();
        input.performance.emotion = emotion;
        assert_eq!(
            LocalSynthesisRequest::from_resolved(&input),
            Err(LocalRequestError::UnsupportedEmotion)
        );
    }
    for intensity in [None, Some(1), Some(1_000)] {
        let mut input = resolved();
        input.performance.intensity_permille = intensity;
        assert_eq!(
            LocalSynthesisRequest::from_resolved(&input),
            Err(LocalRequestError::UnsupportedIntensity)
        );
    }
}

#[test]
fn text_is_bounded_single_line_non_empty_and_nfc() {
    for text in [
        "".to_owned(),
        "  ".to_owned(),
        "private\ntext".to_owned(),
        "private\ttext".to_owned(),
        "private\0text".to_owned(),
        "private\u{2028}text".to_owned(),
        "private\u{2029}text".to_owned(),
        "Đọc [[a]] tiếp".to_owned(),
        "Đọc [[".to_owned(),
        "o\u{31b}".to_owned(),
        "x".repeat(MAX_TEXT_BYTES + 1),
        "ơ".repeat(MAX_TEXT_BYTES / 2 + 1),
    ] {
        let mut input = resolved();
        input.text = text;
        assert_eq!(
            LocalSynthesisRequest::from_resolved(&input),
            Err(LocalRequestError::InvalidText)
        );
    }
    for bytes in [1, MAX_TEXT_BYTES - 1, MAX_TEXT_BYTES] {
        let mut input = resolved();
        input.text = "x".repeat(bytes);
        assert_eq!(
            LocalSynthesisRequest::from_resolved(&input)
                .unwrap()
                .text()
                .len(),
            bytes
        );
    }
}

#[cfg(unix)]
mod process {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    use cantos_api::tts::audio::{AudioError, MAX_OUTPUT_BYTES};
    use cantos_api::wav_inspection::{ChunkId, StructuralError};
    use uuid::Uuid;

    use super::*;

    struct Fixture {
        root: PathBuf,
        runtime: PathBuf,
    }

    impl Fixture {
        fn new(synthesis: &str) -> Self {
            let root = std::env::temp_dir().join(format!("cantos-local-test-{}", Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let root = fs::canonicalize(root).unwrap();
            let runtime = root.join("fake-espeak");
            let fixture = Self { root, runtime };
            fixture.script("1.52.0", "vi", synthesis);
            fs::write(fixture.root.join("fixture.wav"), wav()).unwrap();
            fixture
        }

        fn script(&self, version: &str, voice: &str, synthesis: &str) {
            let file = match voice {
                "vi-vn-x-central" => "aav/vi-VN-x-central",
                "vi-vn-x-south" => "aav/vi-VN-x-south",
                _ => "aav/vi",
            };
            let table = format!(
                "Pty Language Age/Gender VoiceName File\\n5 {voice} --/M Synthetic {file}\\n"
            );
            self.script_table(version, &table, synthesis);
        }

        fn script_table(&self, version: &str, table: &str, synthesis: &str) {
            let root = self.root.to_str().unwrap();
            let script = format!(
                "#!/bin/sh\n[ \"$HOME\" = \"$PWD\" ] && [ \"$TMPDIR\" = \"$PWD\" ] && [ \"$LC_ALL\" = 'C' ] && [ \"$PATH\" = '/usr/bin:/bin' ] || exit 95\n[ -z \"${{ESPEAK_DATA_PATH+x}}\" ] || exit 96\ncase \"$1\" in\n--version) printf 'eSpeak NG text-to-speech: {version} Data at: fixture\\n'; exit 0;;\n--voices=vi) printf '{table}'; exit 0;;\nesac\nprintf '%s\\n' \"$@\" > '{root}/args.txt'\nwhile [ \"$#\" -gt 0 ]; do\nif [ \"$1\" = '-w' ]; then shift; output=$1; fi\nshift\ndone\n/bin/cat > '{root}/stdin.txt'\n{synthesis}\n"
            );
            fs::write(&self.runtime, script).unwrap();
            fs::set_permissions(&self.runtime, fs::Permissions::from_mode(0o700)).unwrap();
        }

        fn run(&self) -> Result<cantos_api::tts::local::LocalSynthesisOutput, LocalRunError> {
            // Successful fixture startup is not a two-second host scheduling benchmark.
            // The separate timeout fixture uses its own short deadline and verifies reaping.
            self.run_for(Duration::from_secs(10))
        }

        fn run_for(
            &self,
            timeout: Duration,
        ) -> Result<cantos_api::tts::local::LocalSynthesisOutput, LocalRunError> {
            run_local(
                &LocalSynthesisRequest::from_resolved(&resolved()).unwrap(),
                &self.runtime,
                &self.root,
                timeout,
            )
        }

        fn assert_clean(&self) {
            for entry in fs::read_dir(&self.root).unwrap() {
                let name = entry.unwrap().file_name();
                assert!(!name.to_string_lossy().starts_with("cantos-local-tts-"));
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn wav() -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend(40_u32.to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(22_050_u32.to_le_bytes());
        bytes.extend(44_100_u32.to_le_bytes());
        bytes.extend(2_u16.to_le_bytes());
        bytes.extend(16_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(4_u32.to_le_bytes());
        bytes.extend([1, 0, 255, 255]);
        bytes
    }

    #[test]
    fn framing_preserves_final_utf8_and_exact_arguments_without_shell_text() {
        let fixture = Fixture::new("/bin/cp ../fixture.wav \"$output\"");
        let output = fixture.run().unwrap();
        assert_eq!(output.bytes(), wav());
        assert_eq!(output.audio().frames(), 2);
        assert_eq!(output.audio().sample_rate_hz(), 22_050);
        assert_eq!(output.audio().channels(), 1);
        assert_eq!(output.audio().bits_per_sample(), 16);
        // Independent Python struct/hashlib construction of this two-frame PCM fixture.
        assert_eq!(
            output.audio().file_sha256(),
            "09be58428331ed9da447d529d3130cc702468da81cd3b361d302d5b92e4f0744"
        );
        assert_eq!(output.runtime_version(), "1.52.0");
        assert_eq!(output.provider_id(), PROVIDER_ID);
        assert_eq!(output.model_id(), MODEL_ID);
        assert_eq!(output.adapter_version(), ADAPTER_VERSION);
        assert_eq!(output.output_contract(), OUTPUT_CONTRACT);
        assert_eq!(
            fs::read(fixture.root.join("stdin.txt")).unwrap(),
            format!("{}\n", resolved().text).as_bytes()
        );
        let args = fs::read_to_string(fixture.root.join("args.txt")).unwrap();
        let args: Vec<_> = args.lines().collect();
        assert_eq!(
            &args[..10],
            ["--stdin", "-v", "vi", "-s", "175", "-p", "50", "-a", "100", "-w"]
        );
        assert_eq!(args.len(), 11);
        assert_eq!(args[10], "output.wav");
        assert!(!fixture.root.join("output.wav").exists());
        fixture.assert_clean();
    }

    #[test]
    fn deep_workspace_passes_only_owned_relative_output_basename() {
        let fixture = Fixture::new("exit 98");
        let mut workspace = fixture.root.clone();
        for _ in 0..4 {
            workspace.push("long-owned-workspace-component-for-native-filename-regression");
        }
        fs::create_dir_all(&workspace).unwrap();
        assert!(workspace.as_os_str().len() > 200);
        fs::write(workspace.join("fixture.wav"), wav()).unwrap();
        // Model the installed engine's native fixed filename buffer independently of the
        // adapter: deep absolute argv output is rejected, while the private cwd stays deep.
        fixture.script("1.52.0", "vi", "[ \"$output\" = 'output.wav' ] || exit 98\n[ \"${#PWD}\" -gt 200 ] || exit 97\n/bin/cp ../fixture.wav \"$output\"");
        let output = run_local(
            &LocalSynthesisRequest::from_resolved(&resolved()).unwrap(),
            &fixture.runtime,
            &workspace,
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(output.bytes(), wav());
        assert_eq!(output.audio().frames(), 2);
        assert_eq!(
            output.audio().file_sha256(),
            "09be58428331ed9da447d529d3130cc702468da81cd3b361d302d5b92e4f0744"
        );
        assert_eq!(fs::read(workspace.join("fixture.wav")).unwrap(), wav());
        assert!(fs::read_dir(&workspace).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("cantos-local-tts-")));
    }

    #[test]
    fn version_and_available_voice_are_required_before_synthesis() {
        for (version, voice, expected) in [
            ("1.51.0", "vi", LocalRunError::WrongRuntimeVersion),
            ("1.52.01", "vi", LocalRunError::WrongRuntimeVersion),
            ("1.52.0", "en", LocalRunError::RuntimeVoiceUnavailable),
        ] {
            let fixture = Fixture::new("exit 0");
            fixture.script(version, voice, "exit 0");
            assert_eq!(fixture.run().unwrap_err(), expected);
            assert!(!fixture.root.join("args.txt").exists());
            fixture.assert_clean();
        }
    }

    #[test]
    fn a_voice_row_with_another_data_file_or_duplicate_language_is_rejected() {
        for table in [
            "Pty Language Age/Gender VoiceName File\\n5 vi --/M Synthetic custom/vi\\n",
            "Pty Language Age/Gender VoiceName File\\n5 vi --/M Synthetic\\n",
            "Pty Language Age/Gender VoiceName File\\n5 vi --/M Synthetic aav/vi\\n5 vi --/M Replacement custom/vi\\n",
        ] {
            let fixture = Fixture::new("exit 0");
            fixture.script_table("1.52.0", table, "exit 0");
            assert_eq!(
                fixture.run().unwrap_err(),
                LocalRunError::RuntimeVoiceUnavailable
            );
            assert!(!fixture.root.join("args.txt").exists());
            fixture.assert_clean();
        }
    }

    #[test]
    fn each_builtin_voice_requires_its_exact_data_file() {
        for voice in ["vi", "vi-vn-x-central", "vi-vn-x-south"] {
            let fixture = Fixture::new("/bin/cp ../fixture.wav \"$output\"");
            fixture.script("1.52.0", voice, "/bin/cp ../fixture.wav \"$output\"");
            let mut input = resolved();
            input.voice_id = voice.into();
            let output = run_local(
                &LocalSynthesisRequest::from_resolved(&input).unwrap(),
                &fixture.runtime,
                &fixture.root,
                Duration::from_secs(10),
            )
            .unwrap();
            assert_eq!(output.bytes(), wav());
            fixture.assert_clean();
        }
    }

    #[test]
    fn runtime_and_workspace_paths_are_explicit_and_validated() {
        let fixture = Fixture::new("exit 0");
        let request = LocalSynthesisRequest::from_resolved(&resolved()).unwrap();
        assert_eq!(
            run_local(
                &request,
                Path::new("fake-espeak"),
                &fixture.root,
                Duration::from_secs(1)
            )
            .unwrap_err(),
            LocalRunError::InvalidRuntimePath
        );
        fs::set_permissions(&fixture.runtime, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            fixture.run().unwrap_err(),
            LocalRunError::InvalidRuntimePath
        );
        fs::set_permissions(&fixture.runtime, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            run_local(
                &request,
                &fixture.runtime,
                Path::new("relative-root"),
                Duration::from_secs(1)
            )
            .unwrap_err(),
            LocalRunError::InvalidWorkspaceRoot
        );
        for timeout in [Duration::ZERO, Duration::from_secs(MAX_RUNTIME_SECONDS + 1)] {
            assert_eq!(
                fixture.run_for(timeout).unwrap_err(),
                LocalRunError::InvalidTimeout
            );
        }
        fixture.assert_clean();
    }

    #[test]
    fn timeout_kills_and_reaps_only_the_owned_child_and_cleans_its_directory() {
        let fixture = Fixture::new("printf '%s\\n' \"$$\" > ../pid.txt\nwhile :; do :; done");
        // The budget includes two preflight children. Give their startup the same
        // host-scheduling headroom as the successful fixtures so parallel tests
        // reach the intentional synthesis hang. The requested total deadline and
        // the owned-child kill/reap assertions remain the oracle.
        let timeout = Duration::from_secs(10);
        let start = Instant::now();
        assert_eq!(
            fixture.run_for(timeout).unwrap_err(),
            LocalRunError::TimedOut {
                phase: LocalPhase::Synthesize
            }
        );
        assert!(start.elapsed() < timeout + Duration::from_secs(2));
        let pid: libc::pid_t = fs::read_to_string(fixture.root.join("pid.txt"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // SAFETY: this is the PID written by the exact synthetic child. WNOHANG
        // only observes/reaps that child; it cannot wait for an unrelated process.
        let result = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
        let error = std::io::Error::last_os_error().raw_os_error();
        if result == 0 {
            // SAFETY: test cleanup is restricted to its recorded, unreaped child.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
                libc::waitpid(pid, std::ptr::null_mut(), 0);
            }
        }
        assert_eq!(result, -1, "runner must already have reaped its child");
        assert_eq!(error, Some(libc::ECHILD));
        fixture.assert_clean();
    }

    #[test]
    fn missing_output_never_reuses_an_existing_unrelated_file() {
        let fixture = Fixture::new("exit 0");
        fs::write(fixture.root.join("output.wav"), wav()).unwrap();
        assert_eq!(
            fixture.run().unwrap_err(),
            LocalRunError::MissingOrUnsafeOutput
        );
        assert_eq!(fs::read(fixture.root.join("output.wav")).unwrap(), wav());
        fixture.assert_clean();
    }

    #[test]
    fn symlink_hardlink_and_fifo_outputs_are_rejected_without_reading_the_target() {
        for synthesis in [
            "/bin/ln -s ../fixture.wav \"$output\"",
            "/bin/ln ../fixture.wav \"$output\"",
            "/usr/bin/mkfifo \"$output\"",
        ] {
            let fixture = Fixture::new(synthesis);
            assert_eq!(
                fixture.run().unwrap_err(),
                LocalRunError::MissingOrUnsafeOutput
            );
            assert_eq!(fs::read(fixture.root.join("fixture.wav")).unwrap(), wav());
            fixture.assert_clean();
        }
    }

    #[test]
    fn empty_truncated_and_wrong_profile_wav_bytes_are_rejected() {
        let fixture = Fixture::new("/bin/cp ../fixture.wav \"$output\"");
        let mut empty = wav();
        empty.truncate(44);
        empty[4..8].copy_from_slice(&36_u32.to_le_bytes());
        empty[40..44].copy_from_slice(&0_u32.to_le_bytes());
        fs::write(fixture.root.join("fixture.wav"), &empty).unwrap();
        assert_eq!(
            fixture.run().unwrap_err(),
            LocalRunError::Audio(AudioError::EmptyFrames)
        );
        let mut truncated = wav();
        truncated.pop();
        fs::write(fixture.root.join("fixture.wav"), &truncated).unwrap();
        assert_eq!(
            fixture.run().unwrap_err(),
            LocalRunError::Audio(AudioError::InvalidWav {
                errors: vec![
                    StructuralError::RiffSizeExceedsFile {
                        riff_end: 48,
                        file_bytes: 47
                    },
                    StructuralError::ChunkOverrun {
                        id: ChunkId::DATA,
                        offset: 36,
                        declared_size: 4,
                        available: 3
                    },
                ],
            })
        );
        let mut wrong_profile = wav();
        wrong_profile[24..28].copy_from_slice(&24_000_u32.to_le_bytes());
        wrong_profile[28..32].copy_from_slice(&48_000_u32.to_le_bytes());
        fs::write(fixture.root.join("fixture.wav"), &wrong_profile).unwrap();
        assert_eq!(
            fixture.run().unwrap_err(),
            LocalRunError::Audio(AudioError::UnsupportedProfile {
                format_tag: 1,
                sample_rate_hz: 24_000,
                channels: 1,
                bits_per_sample: 16,
            })
        );
        fixture.assert_clean();
    }

    #[test]
    fn malformed_bytes_are_rejected_and_oversized_writes_stop_at_the_child_file_limit() {
        let fixture = Fixture::new("/bin/cp ../fixture.wav \"$output\"");
        fs::write(fixture.root.join("fixture.wav"), b"not audio").unwrap();
        assert_eq!(
            fixture.run().unwrap_err(),
            LocalRunError::Audio(AudioError::InvalidWav {
                errors: vec![StructuralError::TruncatedRiffHeader { file_bytes: 9 }],
            })
        );
        fs::write(
            fixture.root.join("fixture.wav"),
            vec![0_u8; MAX_OUTPUT_BYTES + 1],
        )
        .unwrap();
        match fixture.run().unwrap_err() {
            LocalRunError::ProcessFailed { phase, code } => {
                assert_eq!(phase, LocalPhase::Synthesize);
                assert_ne!(code, Some(0));
            }
            other => panic!("file-limit failure must reject the child result: {other:?}"),
        }
        fixture.assert_clean();
    }

    #[test]
    fn process_failure_discards_partial_audio_and_redacts_child_text() {
        let fixture = Fixture::new(
            "/bin/cp ../fixture.wav \"$output\"\nprintf 'PRIVATE_SENTINEL' >&2\nexit 7",
        );
        let error = fixture.run().unwrap_err();
        assert_eq!(
            error,
            LocalRunError::ProcessFailed {
                phase: LocalPhase::Synthesize,
                code: Some(7)
            }
        );
        assert!(!format!("{error:?} {error}").contains("PRIVATE_SENTINEL"));
        fixture.assert_clean();
    }

    #[test]
    fn excessive_child_stdout_or_stderr_is_bounded_and_rejected() {
        for redirection in ["", " >&2"] {
            let body = format!("i=0; while [ \"$i\" -lt 5000 ]; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\\n'{redirection}; i=$((i+1)); done");
            let fixture = Fixture::new(&body);
            assert_eq!(
                fixture.run().unwrap_err(),
                LocalRunError::ProcessOutputExceeded {
                    phase: LocalPhase::Synthesize
                }
            );
            fixture.assert_clean();
        }
    }
}
