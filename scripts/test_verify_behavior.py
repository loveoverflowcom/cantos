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

    def typecheck_fixture(self, variables, module="forge_attempt", definitions=(), qualifier="val"):
        declarations = [{"kind": "var", "name": name, "id": index + 1000}
                        for index, name in enumerate(variables)]
        declarations += [{"kind": "def", "name": name, "id": index, "qualifier": qualifier}
                         for index, name in enumerate(definitions)]
        return {"stage": "typechecking", "errors": [],
                "modules": [{"name": module, "declarations": declarations}],
                "types": {str(row["id"]): {"type": {"kind": "int" if row["kind"] == "var" else "bool"},
                                           "typeVariables": [], "rowVariables": []}
                          for row in declarations}}

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

    def synthetic_forge_campaign(self, trace_mutation=None, type_mutation=None):
        """Exercise the real gate shell with synthetic Quint/process outputs only."""
        root = self.catalog_fixture()
        cap = behavior.load_catalog(root, "cantos.production.forge-attempt")[0]
        receipt = behavior.artifact_root(root) / "installation.json"
        receipt.write_text('{"schemaVersion": 1}\n')
        versions = {"installationReceiptSha256": behavior.digest(receipt)}
        inputs = {source: behavior.digest(root / source) for source in (cap["model"], cap["scenarios"])}
        variables = ["state", "before", "last", "verdict", "ack"]
        labels = []

        def write_trace(path, source, status, count, label):
            data = {"#meta": {"format": "ITF", "source": source, "status": status},
                    "vars": list(variables),
                    "states": [{**{name: {} for name in variables}, "#meta": {"index": i}}
                               for i in range(count)]}
            if trace_mutation:
                trace_mutation(label, data)
            path.write_text(json.dumps(data))

        def launch(argv, _cwd, _timeout=10, _log=None):
            if argv[:2] == ["git", "rev-parse"]:
                return CompletedProcess(argv, 0, "revision\n", "")
            label = Path(argv[-1]).stem
            labels.append(label)
            if argv[3] == "typecheck":
                data = self.typecheck_fixture(variables, cap["module"],
                                              [row["declaration"] for row in cap["properties"] + cap["witnesses"]])
                if label == "scenario-types":
                    scenario = self.typecheck_fixture([], cap["scenarioModule"],
                                                       [row["declaration"] for row in cap["runs"]], "run")
                    data["modules"] += scenario["modules"]
                    data["types"].update(scenario["types"])
                if type_mutation:
                    type_mutation(label, data)
                code = 0
            elif argv[3] == "test":
                names = [row["declaration"] for row in cap["runs"]]
                pattern = argv[argv.index("--out-itf") + 1]
                for name in names:
                    write_trace(Path(pattern.replace("{test}", name).replace("{seq}", "1")),
                                argv[4], "passed", 2, label)
                data = {"stage": "testing", "errors": [], "failed": [], "ignored": [], "passed": names}
                code = 0
            else:
                self.assertEqual(argv[3], "run")
                violation = label != "properties"
                status, code = ("violation", 1) if violation else ("ok", 0)
                count = 2 if violation else cap["simulation"]["steps"] + 1
                write_trace(Path(argv[argv.index("--out-itf") + 1]), argv[4], status, count, label)
                data = {"stage": "running", "status": status, "trace": [{}] * count, "seed": 203, "errors": []}
            Path(argv[-1]).write_text(json.dumps(data))
            return CompletedProcess(argv, code, "", "")

        error = None
        with patch.object(behavior, "process", side_effect=launch), \
                patch.object(behavior, "snapshot", return_value=inputs), \
                patch.object(behavior, "installed_tool", return_value=(versions, {}, None)):
            try:
                behavior.execute_capability(root, cap, versions, {"node": "node"}, Path("cli.js"), inputs, "revision")
            except behavior.BehaviorError as failure:
                error = failure
        bundle = next((root / "target/verification/behavior").glob(cap["id"] + "-*"))
        return cap, behavior.read_json(bundle / "report.json"), bundle, labels, error

    def test_current_catalog_executable_capabilities_and_all_controls(self):
        selected = behavior.load_catalog(behavior.ROOT)
        # Explicit capability counts pin the reviewed selection and each sensitivity control set.
        expected = {
            self.cap["id"]: (19, 7, 11),
            "cantos.media.public-playback-delivery": (12, 5, 10),
            "cantos.media.public-cover-projection": (10, 5, 8),
            "cantos.tooling.hosting-deploy": (6, 3, 3),
            "cantos.presentation.locale-preference": (6, 4, 4),
            "cantos.identity.theatre-registration": (11, 5, 4),
            "cantos.production.forge-attempt": (13, 7, 9),
            "cantos.identity.bearer-lifecycle": (4, 4, 4),
        }
        self.assertEqual([cap["id"] for cap in selected], list(expected))
        for cap in selected:
            with self.subTest(capability=cap["id"]):
                self.assertEqual(
                    (len(cap["runs"]), len(cap["properties"]), len(cap["witnesses"])),
                    expected[cap["id"]])
                self.assertEqual(len(cap["controls"]), len(cap["properties"]))

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

    def test_observation_variables_use_only_the_exact_successful_typechecked_model(self):
        variables = ["state", "before", "last", "verdict", "ack"]
        valid = self.typecheck_fixture(variables)
        valid["modules"].append({"name": "foreign_model", "declarations": [
            {"kind": "var", "name": "foreign", "id": 2000}]})
        self.assertEqual(behavior.observation_variables(valid, "forge_attempt"), sorted(variables))
        mutations = (
            lambda x: x.update(stage="parsing"),
            lambda x: x.update(errors=["type error"]),
            lambda x: x["modules"][0].update(name="wrong_model"),
            lambda x: x["modules"].append(copy.deepcopy(x["modules"][0])),
            lambda x: x["modules"][0].update(declarations=[]),
            lambda x: x["modules"][0]["declarations"].append(copy.deepcopy(x["modules"][0]["declarations"][0])),
            lambda x: x["modules"][0]["declarations"][0].update(name="not a Quint name"),
            lambda x: x["modules"][0]["declarations"][0].update(id=1001),
            lambda x: x["types"].pop("1000"),
            lambda x: x["types"].update({"1000": {}}),
            lambda x: x["types"]["1000"].update(type={}),
            lambda x: x["types"]["1000"].update(type={"kind": "var", "name": "a"}),
            lambda x: x["types"]["1000"].update(typeVariables=["a"]),
        )
        for mutate in mutations:
            malformed = copy.deepcopy(valid)
            mutate(malformed)
            with self.subTest(mutate=mutate), self.assertRaises(behavior.BehaviorError):
                behavior.observation_variables(malformed, "forge_attempt")

    def test_itf_requires_complete_observation_envelope_and_counterexample(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.json"
            variables = ["state", "before", "last", "outcome"]
            source = Path("pilot.qnt")
            valid = {"#meta": {"format": "ITF", "source": str(source), "status": "violation"},
                     "vars": variables, "states": [{**{k: {} for k in variables}, "#meta": {"index": i}} for i in range(2)]}
            path.write_text(json.dumps(valid))
            self.assertEqual(len(behavior.validate_itf(path, source, "violation", 2, expected_variables=variables)), 64)
            for edit in ({"vars": []}, {"states": []}, {"states": [{"state": {}}, {"state": {}}]}):
                path.write_text(json.dumps({**valid, **edit}))
                with self.subTest(edit=edit), self.assertRaises(behavior.BehaviorError):
                    behavior.validate_itf(path, source, "violation", 2, expected_variables=variables)

    def test_itf_accepts_forge_five_variable_observation_envelope(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.json"
            source = Path("forge_attempt_tests.qnt")
            variables = ["state", "before", "last", "verdict", "ack"]
            expected = behavior.observation_variables(self.typecheck_fixture(variables), "forge_attempt")
            data = {"#meta": {"format": "ITF", "source": str(source), "status": "passed"},
                    "vars": variables,
                    "states": [{**{name: {} for name in variables}, "#meta": {"index": i}}
                               for i in range(2)]}
            path.write_text(json.dumps(data))
            self.assertEqual(len(behavior.validate_itf(path, source, "passed", 2, expected_variables=expected)), 64)

    def test_forge_itf_rejects_missing_extra_duplicate_foreign_or_untyped_variables(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.json"
            source = Path("forge_attempt.qnt")
            variables = ["state", "before", "last", "verdict", "ack"]
            expected = behavior.observation_variables(self.typecheck_fixture(variables), "forge_attempt")
            valid = {"#meta": {"format": "ITF", "source": str(source), "status": "violation"},
                     "vars": variables,
                     "states": [{**{name: {} for name in variables}, "#meta": {"index": i}}
                                for i in range(2)]}
            mutations = (
                lambda x: x.update(vars=x["vars"][:-1]),
                lambda x: x["vars"].append("foreign"),
                lambda x: x["vars"].__setitem__(-1, "state"),
                lambda x: x["vars"].__setitem__(-1, "outcome"),
                lambda x: x["vars"].__setitem__(-1, {}),
                lambda x: x["states"][1].pop("ack"),
                lambda x: x["states"][1].update(foreign={}),
            )
            for mutate in mutations:
                malformed = copy.deepcopy(valid)
                mutate(malformed)
                path.write_text(json.dumps(malformed))
                with self.subTest(mutate=mutate), self.assertRaises(behavior.BehaviorError):
                    behavior.validate_itf(path, source, "violation", 2, expected_variables=expected)
            path.write_text(json.dumps(valid))
            for malformed in (None, [], ["ack", "ack"], ["ack", {}]):
                with self.subTest(expected=malformed), self.assertRaises(behavior.BehaviorError):
                    behavior.validate_itf(path, source, "violation", 2, expected_variables=malformed)

    def test_itf_rejects_partial_reordered_wrong_source_or_wrong_status(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.json"
            variables = ["state", "before", "last", "outcome"]
            source = Path("pilot.qnt")
            valid = {"#meta": {"format": "ITF", "source": str(source), "status": "ok"},
                     "vars": variables, "states": [{**{k: {} for k in variables}, "#meta": {"index": i}} for i in range(31)]}
            path.write_text(json.dumps(valid))
            behavior.validate_itf(path, source, "ok", 31, expected_variables=variables)
            for mutation in (lambda x: x.update(states=x["states"][:2]),
                             lambda x: x["states"][1]["#meta"].update(index=0),
                             lambda x: x["#meta"].update(source="foreign.qnt"),
                             lambda x: x["#meta"].update(status="violation")):
                malformed = copy.deepcopy(valid)
                mutation(malformed)
                path.write_text(json.dumps(malformed))
                with self.assertRaises(behavior.BehaviorError):
                    behavior.validate_itf(path, source, "ok", 31, expected_variables=variables)

    def test_forge_campaign_binds_all_trace_kinds_and_reports_typechecked_variables(self):
        cap, report, bundle, labels, error = self.synthetic_forge_campaign()
        self.assertIsNone(error)
        self.assertEqual(report["status"], "PASS")
        self.assertEqual(set(report["stages"].values()), {"PASS"})
        self.assertEqual(report["observationVariables"], ["ack", "before", "last", "state", "verdict"])
        self.assertEqual(len(report["traces"]), len(cap["runs"]) + len(cap["witnesses"]) + len(cap["controls"]) + 1)
        self.assertIn("scenario-types", labels)
        self.assertIn("control-1-types", labels)
        for evidence in report["traces"].values():
            for item in evidence if isinstance(evidence, list) else [evidence]:
                self.assertEqual(sorted(behavior.read_json(bundle / item["file"])["vars"]), report["observationVariables"])

    def test_forge_campaign_rejects_foreign_envelopes_at_every_trace_stage(self):
        witness = behavior.load_catalog(behavior.ROOT, "cantos.production.forge-attempt")[0]["witnesses"][0]
        for label, stage in (("named-runs", "scenarios"), ("properties", "simulation"),
                             ("witness-" + witness["declaration"], "witnesses"), ("control-1", "negativeControls")):
            def mutate(actual, data):
                if actual == label:
                    data["vars"][-1] = "foreign"
            with self.subTest(label=label):
                _, report, _, _, error = self.synthetic_forge_campaign(trace_mutation=mutate)
                self.assertIsNotNone(error)
                self.assertIn("wrong ITF observation variables", str(error))
                self.assertEqual(report["status"], "FAIL")
                self.assertEqual(report["stages"][stage], "FAIL")
                self.assertEqual(report["stages"]["freshness"], "NOT_RUN")

    def test_forge_campaign_rejects_changed_scenario_or_control_variable_declarations(self):
        for label, stage in (("scenario-types", "typecheck"), ("control-1-types", "negativeControls")):
            def mutate(actual, data):
                if actual == label:
                    data["modules"][0]["declarations"][0]["name"] = "foreign"
            with self.subTest(label=label):
                _, report, _, _, error = self.synthetic_forge_campaign(type_mutation=mutate)
                self.assertIsNotNone(error)
                self.assertIn("observation variables differ", str(error))
                self.assertEqual(report["stages"][stage], "FAIL")

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
                    data = self.typecheck_fixture(["state", "before", "last", "outcome"], self.cap["module"],
                                                  [] if scenario else [row["declaration"] for row in rows])
                    if scenario:
                        runs = self.typecheck_fixture([], self.cap["scenarioModule"],
                                                      [row["declaration"] for row in rows], "run")
                        data["modules"] += runs["modules"]
                        data["types"].update(runs["types"])
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
