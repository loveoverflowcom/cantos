"""Codec/identity/comparison contract tests, not live implementation evidence."""

import copy
import json
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch
from pathlib import Path

import verify_behavior as behavior
import verify_behavior_conformance as conformance


def number(value):
    return {"#bigint": str(value)}


def tagged(tag, value=None):
    return {"tag": tag, "value": {"#tup": []} if value is None else value}


def raw_state():
    return {"facts": {"current": True, "now": number(1), "revoked": False},
            "receipt": tagged("Unsettled"), "pending": tagged("Idle"),
            "inspections": number(0), "admissions": number(0), "ownerView": tagged("AwaitingBytes")}


def raw_trace():
    initial = raw_state()
    pending = copy.deepcopy(initial)
    pending.update({"pending": tagged("Inspecting", {"digest": number(0), "length": number(0)}), "inspections": number(1)})
    settled = copy.deepcopy(pending)
    settled.update({"pending": tagged("Idle"), "receipt": tagged("Settled", {"id": number(1), "admittedAt": number(1), "digest": number(0), "length": number(0)}),
                    "admissions": number(1), "ownerView": tagged("OwnerAdmitted")})
    steps = [(initial, initial, tagged("NoEvent"), tagged("NoOutcome")),
             (pending, initial, tagged("Begin", {"digest": number(0), "length": number(0)}), tagged("Inspect")),
             (pending, pending, tagged("Assertion"), tagged("NoOutcome")),
             (settled, pending, tagged("Complete", True), tagged("Admitted")),
             (settled, settled, tagged("Assertion"), tagged("NoOutcome"))]
    return {"#meta": {"format": "ITF", "source": "source.qnt", "status": "passed"}, "vars": ["state", "before", "last", "outcome"],
            "states": [{"#meta": {"index": index}, "state": copy.deepcopy(state), "before": copy.deepcopy(before), "last": action, "outcome": outcome}
                       for index, (state, before, action, outcome) in enumerate(steps)]}


def observer_double(case):
    calls = 0
    checkpoints = []
    for step in case["checkpoints"]:
        calls += step["event"]["tag"] in ("Begin", "Complete")
        checkpoints.append({"index": step["index"], "state": step["state"], "outcome": step["outcome"],
                            "submission": {"synthetic": "codec-only"}, "rendition": None if step["state"]["receipt"] is None else {"synthetic": "comparison-only"},
                            "decisionCalls": calls})
    return {"schemaVersion": 1, "observationSchema": conformance.SCHEMA, "traces": [{"id": case["id"], "checkpoints": checkpoints}]}


class ConformanceContracts(unittest.TestCase):
    def test_exact_itf_and_event_only_replay_input(self):
        steps = conformance.decode_trace(raw_trace())
        self.assertEqual([step["event"]["tag"] for step in steps], ["NoEvent", "Begin", "Assertion", "Complete", "Assertion"])
        self.assertEqual(steps[3]["state"]["receipt"], {"id": 1, "admittedAt": 1, "digest": 0, "length": 0})
        input_data = conformance.replay_input([{"id": "stable.run.id", "checkpoints": steps}])
        self.assertEqual(set(input_data["traces"][0]["checkpoints"][3]), {"index", "event"})
        self.assertNotIn("state", json.dumps(input_data))

    def test_missing_reordered_duplicate_or_broken_before_checkpoints_fail(self):
        edits = [lambda raw: raw["states"].pop(2),
                 lambda raw: raw["states"].reverse(),
                 lambda raw: raw["states"][2]["#meta"].update(index=1),
                 lambda raw: raw["states"][3]["before"].update(admissions=number(10)),
                 lambda raw: raw["states"][2].pop("outcome"),
                 lambda raw: raw["states"].clear()]
        for edit in edits:
            with self.subTest(edit=edit), self.assertRaises(conformance.ConformanceError):
                raw = raw_trace()
                edit(raw)
                conformance.decode_trace(raw)

    def test_unknown_actions_variants_shapes_and_bigints_fail_closed(self):
        edits = [lambda raw: raw["states"][1]["last"].update(tag="Upload"),
                 lambda raw: raw["states"][1]["last"]["value"].update(digest=number(2)),
                 lambda raw: raw["states"][0]["state"]["facts"].update(now=1),
                 lambda raw: raw["states"][0]["state"]["facts"].update(current=1),
                 lambda raw: raw["states"][0]["state"]["facts"].update(now={"#bigint": "01"}),
                 lambda raw: raw["states"][0]["state"].update(receipt=tagged("Unknown")),
                 lambda raw: raw["states"][0]["last"].update(value=None),
                 lambda raw: raw["states"][3]["last"].update(value=1),
                 lambda raw: raw["states"][0].update(unregistered=1)]
        for edit in edits:
            with self.subTest(edit=edit), self.assertRaises(conformance.ConformanceError):
                raw = raw_trace()
                edit(raw)
                conformance.decode_trace(raw)
        for value in ("+1", "-0", "1.0", "1e0", " 1", "9223372036854775808"):
            with self.subTest(value=value), self.assertRaises(conformance.ConformanceError):
                conformance.integer({"#bigint": value})

    def test_rejected_error_is_closed_and_digest_length_aliases_are_independent(self):
        self.assertEqual(conformance.outcome(tagged("Rejected", tagged("OperationReused"))), {"tag": "Rejected", "value": "OperationReused"})
        self.assertEqual(conformance.request({"digest": number(1), "length": number(0)}), {"digest": 1, "length": 0})
        self.assertEqual(conformance.request({"digest": number(0), "length": number(1)}), {"digest": 0, "length": 1})
        with self.assertRaises(conformance.ConformanceError):
            conformance.outcome(tagged("Rejected", tagged("BindingMismatch")))

    def test_effectful_assertion_is_never_a_positive_trace(self):
        for edit in (lambda step: step.update(outcome=tagged("Admitted")),
                     lambda step: step["state"].update(admissions=number(1))):
            raw = raw_trace()
            edit(raw["states"][2])
            with self.assertRaises(conformance.ConformanceError):
                conformance.decode_trace(raw)

    def test_model_mutants_decode_then_semantically_mismatch(self):
        expected = {"id": "stable.control.id", "checkpoints": conformance.decode_trace(raw_trace())}
        observed = observer_double(expected)
        for count, identity in ((2, 2), (8, 1)):
            raw = raw_trace()
            for step in raw["states"][3:]:
                step["state"].update(admissions=number(count))
                step["state"]["receipt"]["value"]["id"] = number(identity)
            raw["states"][4]["before"] = copy.deepcopy(raw["states"][3]["state"])
            mutant = {"id": expected["id"], "checkpoints": conformance.decode_trace(raw)}
            mismatch = conformance.compare([mutant], observed)
            self.assertEqual(mismatch[0]["index"], 3)

    def test_wrong_outcome_and_state_observations_are_detected(self):
        case = {"id": "stable.run.id", "checkpoints": conformance.decode_trace(raw_trace())}
        self.assertEqual(conformance.compare([case], observer_double(case)), [])
        observed = observer_double(case)
        observed["traces"][0]["checkpoints"][1]["outcome"] = {"tag": "Replay", "value": None}
        self.assertEqual(conformance.compare([case], observed)[0]["index"], 1)
        observed = observer_double(case)
        observed["traces"][0]["checkpoints"][3]["state"] = {**case["checkpoints"][3]["state"], "admissions": 2}
        self.assertEqual(conformance.compare([case], observed)[0]["index"], 3)

    def test_missing_reordered_schema_or_effect_observation_rejects(self):
        case = {"id": "stable.run.id", "checkpoints": conformance.decode_trace(raw_trace())}
        edits = [lambda data: data.update(observationSchema="v2"),
                 lambda data: data["traces"].clear(),
                 lambda data: data["traces"][0]["checkpoints"].pop(),
                 lambda data: data["traces"][0]["checkpoints"][2].update(decisionCalls=2),
                 lambda data: data["traces"][0]["checkpoints"][2].update(index=1),
                 lambda data: data["traces"][0]["checkpoints"][0].pop("submission"),
                 lambda data: data["traces"][0]["checkpoints"][4].update(rendition={"changed": True})]
        for edit in edits:
            with self.subTest(edit=edit), self.assertRaises(conformance.ConformanceError):
                observed = observer_double(case)
                edit(observed)
                conformance.compare([case], observed)

    def test_normalized_observation_types_do_not_alias_booleans_and_integers(self):
        case = {"id": "stable.run.id", "checkpoints": conformance.decode_trace(raw_trace())}
        for key, value in (("admissions", True), ("inspections", False)):
            observed = observer_double(case)
            observed["traces"][0]["checkpoints"][3]["state"] = {**case["checkpoints"][3]["state"], key: value}
            with self.assertRaises(conformance.ConformanceError):
                conformance.compare([case], observed)

    def test_implicit_cargo_compiler_and_observer_configuration_is_rejected(self):
        directory = tempfile.TemporaryDirectory(prefix="cantos-compiler-config-contract-")
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "config.toml"
        for content in ("[build]\nrustflags=[]", "[build]\nrustc='different'", "[build]\ntarget='different'",
                        "[target.x86_64-unknown-linux-gnu]\nrunner='different'", "[env]\nRUSTC_WRAPPER='different'",
                        "[env]\nCANTOS_QUINT_OBSERVER_FAULT='ignore-revocation'", "[env]\nRUSTUP_TOOLCHAIN='1.88.0'",
                        "[env]\nPATH='/different/compiler/bin'", "[env]\nRUSTUP_HOME='/different/home'",
                        "[env]\nCARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER='different'",
                        "[env]\nCARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER='different'"):
            path.write_text(content)
            with patch.object(conformance, "configuration_identity", return_value={str(path): behavior.digest(path)}), self.assertRaises(conformance.ConformanceError):
                conformance.compiler_configuration(Path(directory.name), behavior)
        path.write_text("[net]\noffline=true")
        expected = {str(path): behavior.digest(path), str(path.with_name("config")): None}
        with patch.object(conformance, "configuration_identity", return_value=expected):
            self.assertEqual(conformance.compiler_configuration(Path(directory.name), behavior), expected)

    def test_implementation_tool_versions_and_binary_hashes_are_pinned(self):
        directory = tempfile.TemporaryDirectory(prefix="cantos-compiler-tool-contract-")
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.98.1"\n')
        (root / "bin").mkdir()
        (root / "lib").mkdir()
        for name in ("cargo", "rustc"):
            (root / "bin" / name).write_text(name)
        (root / "lib/librustc_driver.so").write_text("driver")
        def probe(argv, _root):
            output = str(root) if argv[1:] == ["--print", "sysroot"] else Path(argv[0]).name + " 1.98.1 (fixture)"
            return SimpleNamespace(returncode=0, stdout=output)
        with patch.object(conformance.shutil, "which", side_effect=lambda name: str(root / "bin" / name)), patch.object(behavior, "process", side_effect=probe):
            identity = conformance.implementation_tools(root, behavior)
            self.assertEqual(identity["cargo"]["binarySha256"], behavior.digest(root / "bin/cargo"))
            self.assertEqual(identity["compilerDrivers"][str(root / "lib/librustc_driver.so")], behavior.digest(root / "lib/librustc_driver.so"))
            for tool in ("cargo", "rustc"):
                for version in ("1.87.0", "1.98.0", "1.98.10", "1.98.1-nightly", "1.99.0"):
                    def wrong_probe(argv, cwd):
                        if Path(argv[0]).name == tool:
                            return SimpleNamespace(returncode=0, stdout=tool + " " + version + " (wrong)")
                        return probe(argv, cwd)
                    with self.subTest(tool=tool, version=version), patch.object(behavior, "process", side_effect=wrong_probe), self.assertRaises(conformance.ConformanceError):
                        conformance.implementation_tools(root, behavior)
        for channel in ("stable", "1.98", "nightly-2026-08-21"):
            (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "' + channel + '"\n')
            with self.subTest(channel=channel), self.assertRaises(conformance.ConformanceError):
                conformance.implementation_tools(root, behavior)

    def test_ambient_observer_and_compiler_environment_is_rejected(self):
        for key in ("CANTOS_QUINT_OBSERVER_FAULT", "CANTOS_QUINT_REPLAY_INPUT", "RUSTC_WRAPPER", "RUSTFLAGS", "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER",
                    "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"):
            with patch.object(behavior, "child_environment", return_value={key: "wrong"}), self.assertRaises(conformance.ConformanceError):
                conformance.observer_environment(behavior, Path("input"), Path("output"))

    def fixture(self):
        directory = tempfile.TemporaryDirectory(prefix="cantos-conformance-contract-")
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        bundle = root / "bundle"
        bundle.mkdir()
        cap = {"id": conformance.CAPABILITY, "observationSchema": conformance.SCHEMA, "implementationConformance": "pure-core-observer",
               "model": "model.qnt", "scenarios": "scenarios.qnt", "simulation": {"scenarioSamples": 1, "seed": "106"},
               "properties": [{"id": "property", "declaration": "prop"}], "runs": [{"id": "run", "declaration": "test"}],
               "witnesses": [{"id": "witness", "declaration": "witness"}],
               "controls": [{"id": "control", "property": "property", "from": "original", "to": "mutated"}]}
        inputs = {"model.qnt": "original", "scenarios.qnt": "named"}
        for path, data in inputs.items():
            (root / path).write_text(data)
        inputs = {path: behavior.digest(root / path) for path in inputs}
        behavior.retain_inputs(root, bundle, inputs)
        (bundle / "installation.json").write_text('{}')
        versions = {"installationReceiptSha256": behavior.digest(bundle / "installation.json")}
        report = {"schemaVersion": 1, "scope": "model-only", "status": "PASS", "capability": cap["id"], "sourceRevision": "revision",
                  "inputs": inputs, "tools": versions, "budget": cap["simulation"], "selection": {key: cap[key] for key in ("properties", "runs", "witnesses")},
                  "controlSelection": [{"id": "control", "property": "property"}],
                  "observationVariables": sorted(["state", "before", "last", "outcome"]),
                  "stages": {key: "PASS" for key in ("typecheck", "scenarios", "simulation", "witnesses", "negativeControls", "freshness")},
                  "installationReceipt": {"file": "installation.json", "sha256": versions["installationReceiptSha256"]}, "traces": {}}
        (bundle / "control-1.qnt").write_text('mutated')
        for key, source, status in (("test", root / "scenarios.qnt", "passed"), ("properties", root / "model.qnt", "ok"),
                                    ("witness-witness", root / "model.qnt", "violation"), ("control-1", bundle / "control-1.qnt", "violation")):
            path = bundle / (key + ".itf.json")
            raw = raw_trace()
            raw["#meta"].update(source=str(source), status=status)
            behavior.write_json(path, raw)
            item = {"file": path.name, "sha256": behavior.digest(path)}
            report["traces"][key] = [item] if key == "test" else item
        return root, cap, bundle, report, inputs, versions

    def test_fresh_complete_identity_is_selected_and_stale_inputs_fail_before_replay(self):
        root, cap, bundle, report, inputs, versions = self.fixture()
        self.assertEqual(len(conformance.collect_cases(root, cap, bundle, report, inputs, versions, "revision", behavior)), 4)
        edits = [lambda cap, report: report.update(sourceRevision="stale"),
                 lambda cap, report: report.update(inputs={}),
                 lambda cap, report: report.update(tools={}),
                 lambda cap, report: report.update(budget={}),
                 lambda cap, report: report.update(selection={}),
                 lambda cap, report: report.pop("observationVariables"),
                 lambda cap, report: report.update(observationVariables=["state", "before", "last", "verdict", "ack"]),
                 lambda cap, report: cap.update(observationSchema="wrong"),
                 lambda cap, report: report["stages"].update(witnesses="NOT_RUN"),
                 lambda cap, report: report["traces"].update(test=[]),
                 lambda cap, report: report["traces"].pop("witness-witness"),
                 lambda cap, report: report["traces"]["properties"].update(sha256="stale")]
        for edit in edits:
            with self.subTest(edit=edit), self.assertRaises(conformance.ConformanceError):
                current_cap, current_report = copy.deepcopy(cap), copy.deepcopy(report)
                edit(current_cap, current_report)
                conformance.collect_cases(root, current_cap, bundle, current_report, inputs, versions, "revision", behavior)

    def test_zero_observer_tests_fail_and_preserve_partial_conformance_report(self):
        root, cap, bundle, report, inputs, versions = self.fixture()
        behavior.write_json(bundle / "report.json", report)
        case = {"id": "run", "checkpoints": conformance.decode_trace(raw_trace()), "sensitivity": False,
                "source": "source", "file": "file", "sha256": "hash", "seed": "106"}
        tools = {"cargo": {"toolchainBinary": "cargo"}, "rustc": {"toolchainBinary": "rustc"}}
        with patch.object(conformance, "snapshot", return_value={}), \
             patch.object(conformance, "compiler_configuration", return_value={}), \
             patch.object(conformance, "implementation_tools", return_value=tools), \
             patch.object(conformance, "collect_cases", return_value=[case]), \
             patch.object(conformance, "observer_environment", return_value={}), \
             patch.object(conformance, "bounded_run", return_value=SimpleNamespace(returncode=0, stdout="test result: ok. 0 passed; 0 failed; 0 ignored;")), \
             self.assertRaises(conformance.ConformanceError):
            conformance.execute(root, cap, bundle, report, versions, inputs, "revision", behavior)
        partial = behavior.read_json(bundle / "conformance.json")
        self.assertEqual(partial["status"], "FAIL")
        self.assertEqual(partial["stages"]["identity"], "PASS")
        self.assertEqual(partial["stages"]["originalTraces"], "FAIL")
        self.assertEqual(partial["stages"]["mutantSensitivity"], "NOT_RUN")
        self.assertEqual(behavior.read_json(bundle / "report.json")["status"], "PASS")

    def test_mutant_source_or_retained_source_tampering_fails(self):
        root, cap, bundle, report, inputs, versions = self.fixture()
        (bundle / "control-1.qnt").write_text("different mutant")
        with self.assertRaises(conformance.ConformanceError):
            conformance.collect_cases(root, cap, bundle, report, inputs, versions, "revision", behavior)
        (bundle / "control-1.qnt").write_text("mutated")
        (bundle / "inputs/model.qnt").write_text("tampered")
        with self.assertRaises(conformance.ConformanceError):
            conformance.collect_cases(root, cap, bundle, report, inputs, versions, "revision", behavior)


if __name__ == "__main__":
    unittest.main()
