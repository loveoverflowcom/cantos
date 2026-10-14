"""Fail-closed contracts for the model-only primitive, not a second business oracle."""
import json
import copy
import os
import tempfile
import unittest
from pathlib import Path
from subprocess import CompletedProcess
from unittest.mock import patch

import verify_behavior as behavior


class BehaviorContracts(unittest.TestCase):
    def setUp(self):
        self.cap = behavior.read_json(behavior.ROOT / behavior.CATALOG)["capabilities"][0]

    def catalog_fixture(self, change=None):
        # These fixtures never create Git markers or a fake installed verifier.
        directory = tempfile.TemporaryDirectory(prefix="cantos-behavior-contract-")
        self.addCleanup(directory.cleanup)
        root = Path(directory.name)
        catalog = behavior.read_json(behavior.ROOT / behavior.CATALOG)
        for cap in catalog["capabilities"]:
            paths = [cap["owner"], cap["inventory"], *cap["sourceRefs"]]
            if cap["model"]:
                paths += [cap["model"], cap["scenarios"]]
            for relative in paths:
                source = behavior.local_path(behavior.ROOT, relative)
                target = root / relative.split("#", 1)[0]
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(source.read_bytes())
        if change:
            change(catalog)
        (root / behavior.CATALOG).write_text(json.dumps(catalog))
        return root

    def test_current_catalog_has_one_executable_pilot_and_all_controls(self):
        selected = behavior.load_catalog(behavior.ROOT)
        self.assertEqual([cap["id"] for cap in selected], [self.cap["id"]])
        self.assertEqual(len(selected[0]["runs"]), 19)
        self.assertEqual(len(selected[0]["properties"]), 7)
        self.assertEqual(len(selected[0]["witnesses"]), 11)

    def test_unknown_or_unmodeled_selection_fails(self):
        for selection in ("cantos.unknown", "cantos.publishing.lifecycle"):
            with self.subTest(selection=selection), self.assertRaises(behavior.BehaviorError):
                behavior.load_catalog(behavior.ROOT, selection)

    def test_catalog_rejects_zero_bool_oversized_or_missing_budgets(self):
        for value in (0, -1, True, 10001, None):
            root = self.catalog_fixture(lambda c: c["capabilities"][0]["simulation"].update(samples=value))
            with self.subTest(value=value), self.assertRaises(behavior.BehaviorError):
                behavior.load_catalog(root)

    def test_catalog_requires_every_control_and_nonempty_selected_declarations(self):
        for kind in ("controls", "properties", "runs", "witnesses"):
            root = self.catalog_fixture(lambda c: c["capabilities"][0].update({kind: []}))
            with self.subTest(kind=kind), self.assertRaises(behavior.BehaviorError):
                behavior.load_catalog(root)

    def test_duplicate_ids_unresolved_lifecycle_and_inert_controls_fail(self):
        edits = [
            lambda c: c["capabilities"][0]["runs"][1].update(id=c["capabilities"][0]["runs"][0]["id"]),
            lambda c: c["capabilities"][0].update(lifecycle="needs-decision"),
            lambda c: c["capabilities"][0]["controls"][0].update(to=c["capabilities"][0]["controls"][0]["from"]),
            lambda c: c["capabilities"][0]["controls"][0].update({"from": "missing substitution"}),
            lambda c: c["capabilities"][0]["controls"][0].update(property="wrong.property"),
        ]
        for edit in edits:
            with self.subTest(edit=edit), self.assertRaises(behavior.BehaviorError):
                behavior.load_catalog(self.catalog_fixture(edit))

    def test_paths_cannot_escape_use_symlinks_or_special_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "file").write_text("valid")
            (root / "link").symlink_to(root / "file")
            os.mkfifo(root / "pipe")
            for relative in ("../outside", "/etc/passwd", "link", "pipe", "./file", "file\\name"):
                with self.subTest(relative=relative), self.assertRaises(behavior.BehaviorError):
                    behavior.local_path(root, relative)

    def test_scenarios_reject_missing_duplicated_skipped_failed_or_wrong_stage(self):
        valid = {"stage": "testing", "passed": ["one", "two"], "failed": [], "ignored": [], "errors": []}
        behavior.validate_scenarios(valid, ["two", "one"])
        for edit in ({"passed": []}, {"passed": ["one", "one"]}, {"ignored": ["two"]},
                     {"failed": ["two"]}, {"errors": ["parse error"]}, {"stage": "typechecking"}):
            with self.subTest(edit=edit), self.assertRaises(behavior.BehaviorError):
                behavior.validate_scenarios({**valid, **edit}, ["one", "two"])

    def test_real_invariant_violation_is_required_for_witness_and_control(self):
        valid = {"stage": "running", "status": "violation", "trace": [{}, {}], "seed": 106, "errors": []}
        behavior.validate_simulation(valid, 1, True, 30)
        for edit, code in (({"status": "ok"}, 0), ({"status": "error"}, 1),
                           ({"errors": ["runtime error"]}, 1), ({"trace": []}, 1),
                           ({"seed": None}, 1), ({}, 0), ({}, 2)):
            with self.subTest(edit=edit, code=code), self.assertRaises(behavior.BehaviorError):
                behavior.validate_simulation({**valid, **edit}, code, True, 30)

    def test_sampled_campaign_must_reach_requested_step_bound(self):
        valid = {"stage": "running", "status": "ok", "trace": [{}] * 31, "seed": 106, "errors": []}
        behavior.validate_simulation(valid, 0, False, 30)
        for edit in ({"trace": [{}]}, {"trace": [{}, {}]}, {"status": "violation"}, {"errors": ["error"]}):
            with self.subTest(edit=edit), self.assertRaises(behavior.BehaviorError):
                behavior.validate_simulation({**valid, **edit}, 0, False, 30)

    def test_missing_selected_property_typecheck_never_passes(self):
        valid = {"stage": "typechecking", "errors": [], "modules": [{"name": "pilot", "declarations": [
            {"kind": "def", "name": "property", "id": 1, "qualifier": "val"}]}],
            "types": {"1": {"type": {"kind": "bool"}}}}
        behavior.validate_types(valid, "pilot", ["property"])
        for module, names in (("wrong", ["property"]), ("pilot", ["deleted"])):
            with self.subTest(module=module), self.assertRaises(behavior.BehaviorError):
                behavior.validate_types(valid, module, names)
        with self.assertRaises(behavior.BehaviorError):
            behavior.validate_types({**valid, "errors": ["type error"]}, "pilot", ["property"])

    def test_itf_requires_complete_observation_envelope_and_counterexample(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.json"
            variables = ["state", "before", "last", "outcome"]
            source = Path("pilot.qnt")
            valid = {"#meta": {"format": "ITF", "source": str(source), "status": "violation"},
                     "vars": variables, "states": [{**{k: {} for k in variables}, "#meta": {"index": i}} for i in range(2)]}
            path.write_text(json.dumps(valid))
            self.assertEqual(len(behavior.validate_itf(path, source, "violation", 2)), 64)
            for edit in ({"vars": []}, {"states": []}, {"states": [{"state": {}}, {"state": {}}]}):
                path.write_text(json.dumps({**valid, **edit}))
                with self.subTest(edit=edit), self.assertRaises(behavior.BehaviorError):
                    behavior.validate_itf(path, source, "violation", 2)

    def test_itf_rejects_partial_reordered_wrong_source_or_wrong_status(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.json"
            variables = ["state", "before", "last", "outcome"]
            source = Path("pilot.qnt")
            valid = {"#meta": {"format": "ITF", "source": str(source), "status": "ok"},
                     "vars": variables, "states": [{**{k: {} for k in variables}, "#meta": {"index": i}} for i in range(31)]}
            path.write_text(json.dumps(valid))
            behavior.validate_itf(path, source, "ok", 31)
            for mutation in (lambda x: x.update(states=x["states"][:2]),
                             lambda x: x["states"][1]["#meta"].update(index=0),
                             lambda x: x["#meta"].update(source="foreign.qnt"),
                             lambda x: x["#meta"].update(status="violation")):
                malformed = copy.deepcopy(valid)
                mutation(malformed)
                path.write_text(json.dumps(malformed))
                with self.assertRaises(behavior.BehaviorError):
                    behavior.validate_itf(path, source, "ok", 31)

    def test_node_preload_is_refused_and_global_module_fallback_is_cleared(self):
        with patch.dict(os.environ, {"NODE_OPTIONS": "--require unreviewed.js"}):
            with self.assertRaises(behavior.BehaviorError):
                behavior.child_environment()
        with patch.dict(os.environ, {"NODE_OPTIONS": "", "NODE_PATH": "/unreviewed/global/modules"}):
            self.assertNotIn("NODE_PATH", behavior.child_environment())

    def test_official_registry_and_integrity_are_checked_before_setup(self):
        manifest = behavior.read_json(behavior.ROOT / behavior.TOOL_DIR / "package.json")
        lock = behavior.read_json(behavior.ROOT / behavior.TOOL_DIR / "package-lock.json")
        behavior.validate_lock(manifest, lock)
        root = self.catalog_fixture()
        tool = root / behavior.TOOL_DIR
        tool.mkdir(parents=True)
        unsafe = copy.deepcopy(lock)
        unsafe["packages"]["node_modules/@informalsystems/quint"]["resolved"] = "https://unrecognized.example/quint.tgz"
        (tool / "package-lock.json").write_text(json.dumps(unsafe))
        with patch.object(behavior, "ROOT", root), patch.object(behavior, "runtime", return_value=(manifest, {}, {"npm": "npm"})), \
                patch.object(behavior, "process") as launch, patch("sys.argv", ["verify_behavior", "--setup"]):
            self.assertEqual(behavior.main(), 1)
            launch.assert_not_called()
        for value in ("https://registry.npmjs.org.evil/quint.tgz", "http://registry.npmjs.org/quint.tgz",
                      "https://registry.npmjs.org/quint.tgz?token=unexpected", "file:///unrecognized/quint.tgz"):
            unsafe["packages"]["node_modules/@informalsystems/quint"]["resolved"] = value
            with self.subTest(value=value), self.assertRaises(behavior.BehaviorError):
                behavior.validate_lock(manifest, unsafe)
        unsafe = copy.deepcopy(lock)
        unsafe["packages"]["node_modules/@informalsystems/quint"]["integrity"] = "sha512-invalid"
        with self.assertRaises(behavior.BehaviorError):
            behavior.validate_lock(manifest, unsafe)

    def test_setup_rejects_a_linked_tool_directory_before_launching_npm(self):
        root = self.catalog_fixture()
        (root / "external-tools").mkdir()
        (root / "external-tools/package.json").write_bytes(
            (behavior.ROOT / behavior.TOOL_DIR / "package.json").read_bytes())
        (root / "tools").mkdir()
        (root / behavior.TOOL_DIR).symlink_to(root / "external-tools", target_is_directory=True)
        with patch.object(behavior, "ROOT", root), patch.object(behavior, "process") as launch, \
                patch("sys.argv", ["verify_behavior", "--setup"]):
            self.assertEqual(behavior.main(), 1)
            launch.assert_not_called()

    def test_artifacts_cannot_follow_an_ancestor_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "elsewhere").mkdir()
            (root / "target").symlink_to(root / "elsewhere", target_is_directory=True)
            with self.assertRaises(behavior.BehaviorError):
                behavior.artifact_root(root)

    def test_installation_receipt_rejects_stale_config_or_installed_content(self):
        inputs = {"package-lock.json": "locked"}
        tools = {"quint": "0.33.0", "installedTreeSha256": "verified-install"}
        receipt = {"schemaVersion": 1, "inputs": inputs, "tools": tools}
        behavior.validate_installation(receipt, inputs, tools)
        for malformed in ({}, {**receipt, "schemaVersion": 2},
                          {**receipt, "inputs": {"package-lock.json": "foreign"}},
                          {**receipt, "tools": {**tools, "installedTreeSha256": "modified"}}):
            with self.subTest(receipt=malformed), self.assertRaises(behavior.BehaviorError):
                behavior.validate_installation(malformed, inputs, tools)

    def test_source_snapshot_detects_changed_model_input(self):
        original = behavior.snapshot(behavior.ROOT, [self.cap])
        self.assertEqual(len(original[self.cap["model"]]), 64)
        with patch.object(behavior, "digest", side_effect=lambda _path: "changed"):
            self.assertNotEqual(original, behavior.snapshot(behavior.ROOT, [self.cap]))

    def test_snapshot_includes_selected_sources_outside_default_model_tree(self):
        cap = {**self.cap, "model": "other_models/model.qnt", "scenarios": "other_models/scenarios.qnt"}
        with patch.object(behavior, "local_path", side_effect=lambda _root, name: Path(name)), \
                patch.object(behavior, "digest", side_effect=lambda path: str(path)):
            original = behavior.snapshot(behavior.ROOT, [cap])
        for selected in (cap["model"], cap["scenarios"]):
            with self.subTest(selected=selected), \
                    patch.object(behavior, "local_path", side_effect=lambda _root, name: Path(name)), \
                    patch.object(behavior, "digest", side_effect=lambda path: "changed" if str(path) == selected else str(path)):
                changed = behavior.snapshot(behavior.ROOT, [cap])
                self.assertNotEqual(original, changed)
                self.assertEqual(changed[selected], "changed")

    def test_input_retention_preserves_dirty_bytes_and_rejects_stale_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "specs/behavior/dirty.qnt"
            source.parent.mkdir(parents=True)
            source.write_text("module dirty {}\n")
            bundle = root / "bundle"
            inputs = {"specs/behavior/dirty.qnt": behavior.digest(source)}
            behavior.retain_inputs(root, bundle, inputs)
            retained = bundle / "inputs/specs/behavior/dirty.qnt"
            source.write_text("module later {}\n")
            self.assertEqual(retained.read_text(), "module dirty {}\n")
            with self.assertRaises(behavior.BehaviorError):
                behavior.retain_inputs(root, root / "stale-bundle", inputs)

    def test_orchestration_preserves_failure_stage_and_never_promotes_missing_evidence(self):
        for mode in ("runtime-error", "missing-trace"):
            root = self.catalog_fixture()
            parent = behavior.artifact_root(root)
            receipt = root / behavior.INSTALLATION
            receipt.write_text('{"schemaVersion": 1}\n')
            versions = {"installationReceiptSha256": behavior.digest(receipt)}
            inputs = {source: behavior.digest(root / source)
                      for source in (self.cap["model"], self.cap["scenarios"])}

            def launch(argv, _cwd, _timeout=10, _log=None):
                if mode == "runtime-error":
                    raise behavior.BehaviorError("synthetic runtime failure")
                if argv[3] == "typecheck":
                    scenario = argv[4] == str(root / self.cap["scenarios"])
                    rows = self.cap["runs"] if scenario else self.cap["properties"] + self.cap["witnesses"]
                    declarations = [{"kind": "def", "name": row["declaration"], "id": i,
                                     "qualifier": "run" if scenario else "val"} for i, row in enumerate(rows)]
                    data = {"stage": "typechecking", "errors": [], "modules": [
                        {"name": self.cap["scenarioModule"] if scenario else self.cap["module"],
                         "declarations": declarations}],
                        "types": {str(i): {"type": {"kind": "bool"}} for i in range(len(rows))}}
                else:
                    # A nominal passing output without any actual ITF cannot pass scenarios.
                    data = {"stage": "testing", "errors": [], "failed": [], "ignored": [],
                            "passed": [row["declaration"] for row in self.cap["runs"]]}
                Path(argv[-1]).write_text(json.dumps(data))
                return CompletedProcess(argv, 0, "", "")

            with self.subTest(mode=mode), patch.object(behavior, "process", side_effect=launch), \
                    self.assertRaises(behavior.BehaviorError):
                behavior.execute_capability(root, self.cap, versions, {"node": "node"}, Path("cli.js"), inputs, "revision")
            report = behavior.read_json(next(parent.glob("*/report.json")))
            self.assertEqual(report["status"], "FAIL")
            self.assertEqual(report["stages"]["typecheck"], "FAIL" if mode == "runtime-error" else "PASS")
            self.assertEqual(report["stages"]["scenarios"], "NOT_RUN" if mode == "runtime-error" else "FAIL")
            self.assertEqual(report["stages"]["simulation"], "NOT_RUN")
            self.assertEqual(report["implementationConformance"], "NOT_RUN")


if __name__ == "__main__":
    unittest.main()
