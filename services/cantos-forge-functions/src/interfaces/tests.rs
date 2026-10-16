//! The command line end to end against the in-memory runtime: arguments in, envelopes out.

use clap::CommandFactory;
use serde_json::{json, Value};

use super::{
    args::Cli,
    cli::{run, ExitStatus, Response, COMMANDS, SCHEMA},
};
use crate::{application::memory::MemoryRuntime, domain::ogg_opus::testing::Fixture};

const WORKSPACE: &str = "/ws";

fn invoke(runtime: &MemoryRuntime, args: &[&str]) -> Response {
    run(
        std::iter::once("cantos-forge").chain(args.iter().copied()),
        runtime,
    )
}

fn json_of(response: &Response) -> Value {
    assert_eq!(response.stderr, "", "a JSON response keeps stderr empty");
    assert_eq!(
        response.stdout.lines().count(),
        1,
        "one envelope per invocation"
    );
    serde_json::from_str(&response.stdout).expect("a JSON envelope")
}

fn audio_ogg() -> Vec<u8> {
    Fixture::mono(vec![30; 50]).build()
}

fn runtime() -> MemoryRuntime {
    MemoryRuntime::default()
        .with_source("/src/story.txt", "Lời hẹn cuối mùa mưa.".as_bytes())
        .with_source("/src/story-v2.txt", "Lời hẹn đã đổi.".as_bytes())
        .with_source("/src/line.ogg", &audio_ogg())
        .with_source("/src/not-audio.ogg", b"definitely not an Ogg stream")
}

fn ok(response: &Response) -> Value {
    let envelope = json_of(response);
    assert_eq!(response.status, ExitStatus::Success, "{envelope}");
    assert_eq!(envelope["outcome"], "ok", "{envelope}");
    envelope["data"].clone()
}

fn problem(response: &Response, outcome: &str) -> Value {
    let envelope = json_of(response);
    assert_eq!(response.status, ExitStatus::Failure, "{envelope}");
    assert_eq!(envelope["outcome"], outcome, "{envelope}");
    assert!(envelope.get("data").is_none());
    envelope["error"].clone()
}

fn init(runtime: &MemoryRuntime) {
    ok(&invoke(
        runtime,
        &["workspace", "init", "--path", WORKSPACE, "--json"],
    ));
}

fn add_story(runtime: &MemoryRuntime) -> String {
    let data = ok(&invoke(
        runtime,
        &[
            "artifact",
            "add",
            "--workspace",
            WORKSPACE,
            "--kind",
            "manuscript-original",
            "--file",
            "/src/story.txt",
            "--producer",
            "AI host session",
            "--rights",
            "original-work",
            "--json",
        ],
    ));
    data["artifact"].as_str().unwrap().to_owned()
}

fn add_audio(runtime: &MemoryRuntime, story: &str, file: &str) -> String {
    let input = format!("{story}@1");
    let data = ok(&invoke(
        runtime,
        &[
            "artifact",
            "add",
            "--workspace",
            WORKSPACE,
            "--kind",
            "audio",
            "--file",
            file,
            "--producer",
            "AI host session",
            "--rights",
            "original-work",
            "--input",
            &input,
            "--json",
        ],
    ));
    data["artifact"].as_str().unwrap().to_owned()
}

#[test]
fn capabilities_report_exactly_what_this_build_does() {
    let data = ok(&invoke(&runtime(), &["capabilities", "--json"]));
    assert_eq!(
        data,
        json!({
            "forge_version": env!("CARGO_PKG_VERSION"),
            "commands": COMMANDS,
            "workflows": { "host_assisted": "local", "provider_managed": "unsupported" },
            "providers": [],
            "handoff": "unsupported",
            "local_bundle_export": "audio",
            "artifact_kinds": [
                "manuscript-original", "manuscript-collected", "script-ir", "audio", "report",
                "instruction-bundle"
            ],
            "audio_profiles": ["opus-mono-20-v1", "opus-mono-24-v1", "opus-mono-32-v1", "opus-mono-40-v1"],
            "checks": ["ogg-opus-profile-v1", "utf8-text-v1"],
        })
    );
}

#[test]
fn the_envelope_has_a_stable_shape_and_the_text_form_carries_the_same_facts() {
    let runtime = runtime();
    let response = invoke(&runtime, &["capabilities", "--json"]);
    let envelope = json_of(&response);
    let keys: Vec<&str> = envelope
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["command", "data", "outcome", "schema"]);
    assert_eq!(envelope["schema"], SCHEMA);
    let text = invoke(&runtime, &["capabilities"]);
    assert_eq!(text.status, ExitStatus::Success);
    assert_eq!(text.stderr, "");
    assert!(text
        .stdout
        .contains("provider-managed workflow: unsupported"));
    assert!(text
        .stdout
        .contains("handoff to the Cantos API: unsupported"));
}

/// Every leaf command path in the clap tree, as typed on the command line.
fn leaf_commands(command: &clap::Command, prefix: &str, out: &mut Vec<String>) {
    for sub in command.get_subcommands() {
        let path = if prefix.is_empty() {
            sub.get_name().to_owned()
        } else {
            format!("{prefix} {}", sub.get_name())
        };
        if sub.has_subcommands() {
            leaf_commands(sub, &path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn the_reported_commands_are_exactly_the_implemented_leaf_commands() {
    let mut implemented = Vec::new();
    leaf_commands(&Cli::command(), "", &mut implemented);
    implemented.sort();
    let mut reported: Vec<String> = COMMANDS
        .iter()
        .map(|command| (*command).to_owned())
        .collect();
    reported.sort();
    assert_eq!(implemented, reported);
}

#[test]
fn usage_errors_exit_2_and_never_write_a_result() {
    let runtime = runtime();
    for args in [
        &["publish"][..],
        &["help"][..],
        &["capabilities", "--yaml"][..],
        &["artifact", "show", "--workspace", WORKSPACE][..],
        &[
            "artifact",
            "show",
            "--workspace",
            WORKSPACE,
            "--artifact",
            "nope",
        ][..],
        &[
            "validate",
            "--workspace",
            WORKSPACE,
            "--artifact",
            "art_00000000000000000000000000000001",
            "--profile",
            "aac",
        ][..],
    ] {
        let response = invoke(&runtime, args);
        assert_eq!(response.status, ExitStatus::Usage, "{args:?}");
        assert_eq!(response.stdout, "", "{args:?}");
        assert!(
            response.stderr.starts_with("error: "),
            "{args:?}: {}",
            response.stderr
        );
    }
}

#[test]
fn a_missing_command_prints_usage_to_stderr_and_exits_2() {
    let response = invoke(&runtime(), &[]);
    assert_eq!(response.status, ExitStatus::Usage);
    assert_eq!(response.stdout, "");
    assert!(response.stderr.contains("Usage: cantos-forge"));
}

#[test]
fn a_json_usage_error_is_a_typed_envelope() {
    let response = invoke(&runtime(), &["submit", "--json"]);
    assert_eq!(response.status, ExitStatus::Usage);
    let envelope = json_of(&response);
    assert_eq!(envelope["schema"], SCHEMA);
    assert_eq!(envelope["command"], Value::Null);
    assert_eq!(envelope["outcome"], "usage_error");
    assert_eq!(envelope["error"]["code"], "usage");
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unrecognized subcommand 'submit'"));
    assert_eq!(envelope.as_object().unwrap().len(), 4);
}

#[test]
fn help_and_version_succeed_on_stdout() {
    for flag in ["--help", "--version"] {
        let response = invoke(&runtime(), &[flag]);
        assert_eq!(response.status, ExitStatus::Success, "{flag}");
        assert!(response.stdout.contains("cantos-forge"), "{flag}");
        assert_eq!(response.stderr, "", "{flag}");
    }
}

#[test]
fn exit_codes_are_stable() {
    let codes = [ExitStatus::Success, ExitStatus::Failure, ExitStatus::Usage].map(ExitStatus::code);
    assert_eq!(codes, [0, 1, 2]);
}

#[test]
fn a_command_on_a_missing_workspace_is_blocked_and_names_the_next_command() {
    let runtime = runtime();
    for args in [
        &["artifact", "list", "--workspace", WORKSPACE, "--json"][..],
        &[
            "artifact",
            "show",
            "--workspace",
            WORKSPACE,
            "--artifact",
            "art_00000000000000000000000000000001",
            "--json",
        ][..],
    ] {
        let error = problem(&invoke(&runtime, args), "blocked");
        assert_eq!(error["code"], "workspace_missing");
        assert_eq!(error["action"], "cantos-forge workspace init --path /ws");
    }
    let text = invoke(&runtime, &["artifact", "list", "--workspace", WORKSPACE]);
    assert_eq!(text.status, ExitStatus::Failure);
    assert_eq!(text.stdout, "");
    assert!(text
        .stderr
        .contains("action: cantos-forge workspace init --path /ws"));
}

#[test]
fn init_is_idempotent() {
    let runtime = runtime();
    let first = ok(&invoke(
        &runtime,
        &["workspace", "init", "--path", WORKSPACE, "--json"],
    ));
    assert_eq!(first, json!({ "workspace": WORKSPACE, "created": true }));
    let again = ok(&invoke(
        &runtime,
        &["workspace", "init", "--path", WORKSPACE, "--json"],
    ));
    assert_eq!(again, json!({ "workspace": WORKSPACE, "created": false }));
}

#[test]
fn register_validate_and_export_a_story_and_its_audio_end_to_end() {
    let runtime = runtime();
    init(&runtime);
    let story = add_story(&runtime);
    let audio = add_audio(&runtime, &story, "/src/line.ogg");
    assert_ne!(story, audio);

    let listed = ok(&invoke(
        &runtime,
        &["artifact", "list", "--workspace", WORKSPACE, "--json"],
    ));
    assert_eq!(listed["artifacts"].as_array().unwrap().len(), 2);

    // Not exportable yet: the profile check has not run.
    let refused = problem(
        &invoke(
            &runtime,
            &[
                "handoff",
                "export",
                "--workspace",
                WORKSPACE,
                "--artifact",
                &audio,
                "--profile",
                "opus-mono-24-v1",
                "--out",
                "/out/a",
                "--json",
            ],
        ),
        "refused",
    );
    assert_eq!(refused["code"], "export_refused");
    assert_eq!(refused["details"][0]["code"], "check_not_run");
    assert_eq!(
        refused["details"][0]["action"],
        format!(
            "cantos-forge validate --workspace /ws --artifact {audio} --profile opus-mono-24-v1"
        )
    );

    // Audio is never measured without a producer-chosen profile.
    let missing = problem(
        &invoke(
            &runtime,
            &[
                "validate",
                "--workspace",
                WORKSPACE,
                "--artifact",
                &audio,
                "--json",
            ],
        ),
        "refused",
    );
    assert_eq!(missing["code"], "profile_required");

    let validated = ok(&invoke(
        &runtime,
        &[
            "validate",
            "--workspace",
            WORKSPACE,
            "--artifact",
            &audio,
            "--profile",
            "opus-mono-24-v1",
            "--json",
        ],
    ));
    assert_eq!(validated["result"], "recorded");
    assert_eq!(validated["outcome"], "passed");
    assert_eq!(validated["check"], "ogg-opus-profile-v1:opus-mono-24-v1");
    assert_eq!(validated["detail"]["container"], "ogg");
    assert_eq!(validated["detail"]["channels"], 1);
    let again = ok(&invoke(
        &runtime,
        &[
            "validate",
            "--workspace",
            WORKSPACE,
            "--artifact",
            &audio,
            "--profile",
            "opus-mono-24-v1",
            "--json",
        ],
    ));
    assert_eq!(again["result"], "already_recorded");
    assert_eq!(again["evidence"], validated["evidence"]);

    let shown = ok(&invoke(
        &runtime,
        &[
            "artifact",
            "show",
            "--workspace",
            WORKSPACE,
            "--artifact",
            &audio,
            "--json",
        ],
    ));
    assert_eq!(shown["kind"], "audio");
    assert_eq!(shown["inputs"]["status"], "current");
    assert_eq!(
        shown["evaluation"]["deterministic"],
        json!({"status": "current", "result": "passed"})
    );
    assert_eq!(
        shown["evaluation"]["ai_review"],
        json!({"status": "not_run"})
    );
    assert_eq!(
        shown["evaluation"]["human_review"],
        json!({"status": "not_run"})
    );
    assert_eq!(shown["evidence"].as_array().unwrap().len(), 1);

    let exported = ok(&invoke(
        &runtime,
        &[
            "handoff",
            "export",
            "--workspace",
            WORKSPACE,
            "--artifact",
            &audio,
            "--profile",
            "opus-mono-24-v1",
            "--out",
            "/out/a",
            "--json",
        ],
    ));
    assert_eq!(exported["submission"], "not_run");
    assert_eq!(exported["bundle"]["audio"], "/out/a/audio.ogg");
    let bundles = runtime.bundles.borrow();
    assert_eq!(bundles["/out/a"].audio, audio_ogg());
    drop(bundles);

    let occupied = problem(
        &invoke(
            &runtime,
            &[
                "handoff",
                "export",
                "--workspace",
                WORKSPACE,
                "--artifact",
                &audio,
                "--profile",
                "opus-mono-24-v1",
                "--out",
                "/out/a",
                "--json",
            ],
        ),
        "refused",
    );
    assert_eq!(occupied["code"], "bundle_destination_exists");
}

#[test]
fn a_changed_script_makes_the_audio_stale_everywhere_it_is_reported() {
    let runtime = runtime();
    init(&runtime);
    let story = add_story(&runtime);
    let audio = add_audio(&runtime, &story, "/src/line.ogg");
    ok(&invoke(
        &runtime,
        &[
            "validate",
            "--workspace",
            WORKSPACE,
            "--artifact",
            &audio,
            "--profile",
            "opus-mono-24-v1",
            "--json",
        ],
    ));
    let revised = ok(&invoke(
        &runtime,
        &[
            "artifact",
            "add",
            "--workspace",
            WORKSPACE,
            "--kind",
            "manuscript-original",
            "--file",
            "/src/story-v2.txt",
            "--producer",
            "AI host session",
            "--rights",
            "original-work",
            "--artifact",
            &story,
            "--json",
        ],
    ));
    assert_eq!(
        (revised["result"].as_str(), revised["revision"].as_u64()),
        (Some("created"), Some(2))
    );

    let shown = ok(&invoke(
        &runtime,
        &[
            "artifact",
            "show",
            "--workspace",
            WORKSPACE,
            "--artifact",
            &audio,
            "--json",
        ],
    ));
    assert_eq!(shown["inputs"]["status"], "stale");
    assert_eq!(shown["inputs"]["superseded"][0]["head"], 2);
    // The audio's own evidence is untouched: it is still about the audio's own revision.
    assert_eq!(shown["evaluation"]["deterministic"]["status"], "current");

    let refused = problem(
        &invoke(
            &runtime,
            &[
                "handoff",
                "export",
                "--workspace",
                WORKSPACE,
                "--artifact",
                &audio,
                "--profile",
                "opus-mono-24-v1",
                "--out",
                "/out/b",
                "--json",
            ],
        ),
        "refused",
    );
    assert_eq!(refused["details"][0]["code"], "inputs_stale");
    assert!(
        runtime.bundles.borrow().is_empty(),
        "a refused export writes nothing"
    );
}

#[test]
fn audio_that_fails_its_profile_is_recorded_and_reported_as_check_failed() {
    let runtime = runtime();
    init(&runtime);
    let bad = ok(&invoke(
        &runtime,
        &[
            "artifact",
            "add",
            "--workspace",
            WORKSPACE,
            "--kind",
            "audio",
            "--file",
            "/src/not-audio.ogg",
            "--producer",
            "Người thu âm",
            "--rights",
            "unstated",
            "--json",
        ],
    ));
    let artifact = bad["artifact"].as_str().unwrap().to_owned();
    let response = invoke(
        &runtime,
        &[
            "validate",
            "--workspace",
            WORKSPACE,
            "--artifact",
            &artifact,
            "--profile",
            "opus-mono-40-v1",
            "--json",
        ],
    );
    assert_eq!(response.status, ExitStatus::Failure);
    let envelope = json_of(&response);
    assert_eq!(envelope["outcome"], "check_failed");
    assert_eq!(envelope["data"]["outcome"], "failed");
    assert_eq!(envelope["data"]["detail"]["refusal"], "not_ogg");
    let shown = ok(&invoke(
        &runtime,
        &[
            "artifact",
            "show",
            "--workspace",
            WORKSPACE,
            "--artifact",
            &artifact,
            "--json",
        ],
    ));
    assert_eq!(
        shown["evaluation"]["deterministic"],
        json!({"status": "current", "result": "failed"})
    );
}

#[test]
fn rights_flags_are_cross_checked_before_anything_is_read() {
    let runtime = runtime();
    init(&runtime);
    let base = [
        "artifact",
        "add",
        "--workspace",
        WORKSPACE,
        "--kind",
        "manuscript-original",
        "--file",
        "/src/story.txt",
        "--producer",
        "AI host session",
    ];
    let with = |extra: &[&str]| {
        let args: Vec<&str> = base.iter().copied().chain(extra.iter().copied()).collect();
        invoke(&runtime, &args)
    };
    for extra in [
        &["--rights", "licensed"][..],
        &["--rights", "permitted-source"][..],
        &["--rights", "original-work", "--rights-reference", "CC-BY"][..],
        &["--rights", "licensed", "--rights-reference", " padded "][..],
    ] {
        let response = with(extra);
        assert_eq!(response.status, ExitStatus::Usage, "{extra:?}");
        assert_eq!(response.stdout, "");
    }
    let licensed = with(&[
        "--rights",
        "licensed",
        "--rights-reference",
        "Giấy phép CC-BY 4.0",
        "--json",
    ]);
    assert_eq!(ok(&licensed)["result"], "created");
}

#[test]
fn refusals_are_typed_and_write_nothing() {
    let runtime = runtime();
    init(&runtime);
    let story = add_story(&runtime);
    let ghost = "art_000000000000000000000000000000ff";
    let add = |extra: &[&str]| {
        let mut args = vec![
            "artifact",
            "add",
            "--workspace",
            WORKSPACE,
            "--kind",
            "audio",
            "--file",
            "/src/line.ogg",
            "--producer",
            "AI host session",
            "--rights",
            "original-work",
            "--json",
        ];
        args.extend_from_slice(extra);
        invoke(&runtime, &args)
    };
    assert_eq!(
        problem(&add(&["--artifact", ghost]), "refused")["code"],
        "unknown_artifact"
    );
    let unknown_input = format!("{story}@9");
    assert_eq!(
        problem(&add(&["--input", &unknown_input]), "refused")["code"],
        "unknown_input"
    );
    let pinned = format!("{story}@1");
    assert_eq!(
        problem(&add(&["--input", &pinned, "--input", &pinned]), "refused")["code"],
        "repeated_input"
    );
    // The story is a manuscript; registering audio bytes as its next revision changes its kind.
    let kind_change = problem(&add(&["--artifact", &story]), "refused");
    assert_eq!(kind_change["code"], "revision_refused");
    let missing_file = invoke(
        &runtime,
        &[
            "artifact",
            "add",
            "--workspace",
            WORKSPACE,
            "--kind",
            "report",
            "--file",
            "/src/none",
            "--producer",
            "x",
            "--rights",
            "unstated",
            "--json",
        ],
    );
    assert_eq!(
        problem(&missing_file, "refused")["code"],
        "source_not_a_regular_file"
    );
    let list = ok(&invoke(
        &runtime,
        &["artifact", "list", "--workspace", WORKSPACE, "--json"],
    ));
    assert_eq!(
        list["artifacts"].as_array().unwrap().len(),
        1,
        "only the story exists"
    );
}

#[test]
fn validate_refuses_kinds_it_has_no_check_for_and_text_takes_no_profile() {
    let runtime = runtime()
        .with_source("/src/report.json", b"{}")
        .with_source("/src/script.json", b"{}");
    init(&runtime);
    let story = add_story(&runtime);
    let register = |kind: &str, file: &str| {
        ok(&invoke(
            &runtime,
            &[
                "artifact",
                "add",
                "--workspace",
                WORKSPACE,
                "--kind",
                kind,
                "--file",
                file,
                "--producer",
                "AI host session",
                "--rights",
                "original-work",
                "--json",
            ],
        ))["artifact"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let report = register("report", "/src/report.json");
    let script = register("script-ir", "/src/script.json");
    for artifact in [&report, &script] {
        let error = problem(
            &invoke(
                &runtime,
                &[
                    "validate",
                    "--workspace",
                    WORKSPACE,
                    "--artifact",
                    artifact,
                    "--json",
                ],
            ),
            "refused",
        );
        assert_eq!(error["code"], "no_check_for_kind");
    }
    let with_profile = problem(
        &invoke(
            &runtime,
            &[
                "validate",
                "--workspace",
                WORKSPACE,
                "--artifact",
                &story,
                "--profile",
                "opus-mono-24-v1",
                "--json",
            ],
        ),
        "refused",
    );
    assert_eq!(with_profile["code"], "profile_not_applicable");
    let text = ok(&invoke(
        &runtime,
        &[
            "validate",
            "--workspace",
            WORKSPACE,
            "--artifact",
            &story,
            "--json",
        ],
    ));
    assert_eq!(text["check"], "utf8-text-v1");
    assert_eq!(text["outcome"], "passed");
}

#[test]
fn only_audio_can_be_exported() {
    let runtime = runtime();
    init(&runtime);
    let story = add_story(&runtime);
    let error = problem(
        &invoke(
            &runtime,
            &[
                "handoff",
                "export",
                "--workspace",
                WORKSPACE,
                "--artifact",
                &story,
                "--profile",
                "opus-mono-24-v1",
                "--out",
                "/out/s",
                "--json",
            ],
        ),
        "refused",
    );
    assert_eq!(error["details"][0]["code"], "not_audio");
    assert_eq!(error["details"][0]["kind"], "manuscript");
}

#[test]
fn the_text_form_of_a_failure_goes_to_stderr_and_leaves_stdout_empty() {
    let runtime = runtime();
    init(&runtime);
    let response = invoke(
        &runtime,
        &[
            "artifact",
            "show",
            "--workspace",
            WORKSPACE,
            "--artifact",
            "art_000000000000000000000000000000ff",
        ],
    );
    assert_eq!(response.status, ExitStatus::Failure);
    assert_eq!(response.stdout, "");
    assert!(
        response.stderr.starts_with("cantos-forge: "),
        "{}",
        response.stderr
    );
}

#[test]
fn a_refused_export_lists_every_reason_and_its_fix_in_text_too() {
    let runtime = runtime();
    init(&runtime);
    let story = add_story(&runtime);
    let audio = add_audio(&runtime, &story, "/src/line.ogg");
    let response = invoke(
        &runtime,
        &[
            "handoff",
            "export",
            "--workspace",
            WORKSPACE,
            "--artifact",
            &audio,
            "--profile",
            "opus-mono-24-v1",
            "--out",
            "/out/t",
        ],
    );
    assert_eq!(response.status, ExitStatus::Failure);
    assert_eq!(response.stdout, "");
    assert!(
        response
            .stderr
            .contains("cannot be exported yet (1 reason)"),
        "{}",
        response.stderr
    );
    assert!(
        response.stderr.contains(&format!(
            "  - check_not_run: cantos-forge validate --workspace /ws --artifact {audio} --profile opus-mono-24-v1"
        )),
        "{}",
        response.stderr
    );
}
