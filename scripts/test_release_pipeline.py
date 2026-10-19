"""Release DAG and production orchestration with deterministic command doubles."""
import argparse
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_pipeline as release
from test_release_artifact import ENV, ORIGIN, SHA, fixture, request


ROOT = Path(__file__).resolve().parents[1]


class PipelineTests(unittest.TestCase):
    def test_real_git_branch_movement_never_changes_the_resolved_release_checkout(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            root = base / "repo"
            root.mkdir()
            def git(*argv):
                return subprocess.run(["git", *argv], cwd=root, check=True, text=True,
                                      stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.strip()
            git("init", "-b", "develop")
            git("config", "user.name", "Release fixture")
            git("config", "user.email", "release-fixture@example.invalid")
            commits = []
            for text in ("release", "config", "branch-moved"):
                (root / "fixture.txt").write_text(text)
                git("add", "fixture.txt")
                git("commit", "-m", text)
                commits.append(git("rev-parse", "HEAD"))
            git("remote", "add", "origin", str(root))
            git("checkout", "--detach", commits[1])
            req = request()
            req.update(source_sha=commits[0], config_sha=commits[1], initial_checkout_sha=commits[1])
            transfer = base / "transfer"
            transfer.mkdir()
            contract.write_new(transfer / "request.json", req)
            env = dict(ENV, CIRCLE_SHA1=commits[1], CANTOS_CONFIG_SHA=commits[1])
            with patch.dict(os.environ, env, clear=True):
                release.checkout(root, transfer)
                self.assertEqual(git("rev-parse", "HEAD"), commits[0])
                self.assertEqual(git("rev-parse", "refs/remotes/origin/develop"), commits[2])
                self.assertEqual(os.environ["CIRCLE_SHA1"], commits[1])
                with self.assertRaisesRegex(ValueError, "stale release"):
                    release.assert_fresh(root, req)

    @patch.dict(os.environ, ENV, clear=True)
    @patch.object(release, "fetch")
    @patch.object(release.release_build, "source_revision", return_value="b" * 40)
    def test_target_is_resolved_once_and_unmerged_target_fails_at_ci_boundary(self, source, fetch):
        values = [None, None, SHA, None]
        with patch.object(release, "run", side_effect=values) as run, patch.object(subprocess, "run") as exists:
            exists.return_value.returncode = 0
            req = release.resolve(ROOT, "some-branch", "all", ORIGIN, False)
            self.assertEqual(req["source_sha"], SHA)
            self.assertEqual(req["config_sha"], "b" * 40)
            self.assertEqual(sum("rev-parse" in c.args[1] for c in run.call_args_list), 1)
            self.assertEqual(run.call_args_list[-1].args[1], ["git", "merge-base", "--is-ancestor", SHA, "refs/remotes/origin/develop"])
        with patch.object(release, "run", side_effect=[None, None, SHA, subprocess.CalledProcessError(1, "merge-base")]), patch.object(subprocess, "run") as exists:
            exists.return_value.returncode = 0
            with self.assertRaises(subprocess.CalledProcessError):
                release.resolve(ROOT, "some-branch", "api", ORIGIN, False)

    def test_conflicting_or_empty_requests_fail_instead_of_successful_noop(self):
        for target, build in [("develop", True), (None, False), ("", False)]:
            with self.subTest(target=target, build=build), patch.object(release, "run") as run:
                with self.assertRaisesRegex(ValueError, "empty target"):
                    release.resolve(ROOT, target, "all", ORIGIN, build)
                run.assert_not_called()

    def test_workspace_rerun_ambiguity_is_refused_without_mtime_selection(self):
        with tempfile.TemporaryDirectory() as temporary:
            transfer = Path(temporary)
            for name in ("old", "new"):
                (transfer / name).mkdir()
                contract.write_new(transfer / name / "request.json", request())
            with self.assertRaisesRegex(ValueError, "exactly one"):
                release.one(transfer, "request.json")

    @patch.object(release, "fetch")
    def test_moved_develop_blocks_stale_promotion(self, fetch):
        with patch.object(release, "run", return_value="c" * 40):
            with self.assertRaisesRegex(ValueError, "stale release"):
                release.assert_fresh(ROOT, request())

    @patch.dict(os.environ, ENV, clear=True)
    def test_partial_release_keeps_successful_component_and_missing_write_unknown(self):
        with tempfile.TemporaryDirectory() as temporary:
            transfer = Path(temporary)
            req = request()
            req["components"] = ["api", "theatre-web", "studio-web"]
            contract.write_new(transfer / "request.json", req)
            for component in ("api", "theatre-web"):
                out = transfer / "outcomes" / component
                out.mkdir(parents=True)
                contract.write_new(out / "outcome.json", {"outcome": "activated", "smoke": "PASS", "remote_identity": component})
            with self.assertRaisesRegex(ValueError, "partial"):
                release.report(transfer)
            summary = contract.read_json(next(transfer.glob("summary-*.json")))
            self.assertEqual(summary["components"]["theatre-web"]["outcome"], "activated")
            self.assertEqual(summary["components"]["studio-web"]["outcome"], "NOT_RUN/unknown")
            self.assertFalse(summary["atomic_release"])

    @patch.dict(os.environ, ENV, clear=True)
    def test_extract_uses_validated_archive_without_trunk_or_product_compilation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            req = request()
            path, manifest = fixture(root, req)
            rp = root / "request.json"
            contract.write_new(rp, req)
            args = argparse.Namespace(release_manifest=path, release_digest=contract.file_record(path, path.parent)["sha256"],
                release_sha=SHA, release_request=rp, local_artifact=False, component="theatre-web", api_origin=ORIGIN, output=root / "dist")
            with patch.object(contract, "configuration", return_value=manifest["configuration"]), patch.object(release.release_build, "source_revision", return_value=SHA), patch.object(release, "run") as run:
                release.extract_web(root, args)
                run.assert_not_called()
            self.assertEqual((args.output / "app.wasm").read_bytes(), b"wasm fixture")
            with self.assertRaises(FileExistsError):
                contract.extract(path.parent / "theatre-web.tar.gz", args.output, web=True)

    @patch.dict(os.environ, dict(ENV, CIRCLE_WORKFLOW_JOB_ID="publisher-job", CIRCLE_BUILD_NUM="456", FIREBASE_PROJECT="fixture-project"), clear=True)
    def test_persisted_intent_is_bound_to_one_job_and_lost_publish_response_never_replays(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            transfer = root / "artifacts/release-transfer"
            transfer.mkdir(parents=True)
            req = request()
            manifest, _ = fixture(transfer, req)
            contract.write_new(transfer / "request.json", req)
            contract.write_new(transfer / "inventory.json", {"schema_version": 1,
                "request_sha256": contract.fingerprint(req), "components": {"theatre-web": {
                    "manifest": manifest.relative_to(transfer).as_posix(),
                    "digest": contract.file_record(manifest, manifest.parent)["sha256"]}}})
            with patch.object(release.release_build, "source_revision", return_value=SHA), patch.object(release, "assert_fresh"):
                release.intent(root, transfer, "theatre-web")
                intent_path = transfer / "intents/theatre-web/intent.json"
                saved = contract.read_json(intent_path)
                self.assertEqual(saved["publisher"]["job_number"], "456")
                # A CI rerun inherits the durable intent, but has a new execution number.
                with patch.dict(os.environ, {"CIRCLE_BUILD_NUM": "457"}), patch.object(release, "run") as run:
                    with self.assertRaisesRegex(ValueError, "rerun publisher intent"):
                        release.promote(root, transfer, "theatre-web")
                    run.assert_not_called()
                def publish_then_lose_response(root, argv, **kwargs):
                    if argv[0] == "cargo":
                        path = root / "artifacts/hosting/observed/receipt.json"
                        path.parent.mkdir(parents=True)
                        contract.write_new(path, {"remote": {"state": "confirmed", "release": {"version": "exact-version"}}, "smoke": None})
                        raise subprocess.CalledProcessError(1, "mock publisher after response")
                with patch.object(release, "run", side_effect=publish_then_lose_response) as run:
                    with self.assertRaises(subprocess.CalledProcessError):
                        release.promote(root, transfer, "theatre-web")
                    self.assertEqual(sum(c.args[1][0] == "cargo" for c in run.call_args_list), 1)
                outcome = contract.read_json(transfer / "outcomes/theatre-web/outcome.json")
                self.assertEqual(outcome["outcome"], "activated")
                self.assertEqual(outcome["command_status"], "FAIL")
                self.assertEqual(outcome["smoke"], "NOT_RUN")
                with patch.object(release, "run") as run:
                    with self.assertRaisesRegex(ValueError, "already attempted"):
                        release.promote(root, transfer, "theatre-web")
                    run.assert_not_called()
                with self.assertRaisesRegex(ValueError, "intent already exists"):
                    release.intent(root, transfer, "theatre-web")


class CircleConfigTests(unittest.TestCase):
    def setUp(self):
        # JSON is a YAML subset; stdlib parsing avoids adding a runtime YAML dependency.
        self.config = json.loads((ROOT / ".circleci/config.yml").read_text())

    def value(self, expr, parameters):
        if isinstance(expr, str) and expr.startswith("<< pipeline.parameters."):
            return parameters[expr.split(".")[-1].split()[0]]
        if isinstance(expr, dict):
            key, value = next(iter(expr.items()))
            if key == "and":
                return all(self.value(v, parameters) for v in value)
            if key == "not":
                return not self.value(value, parameters)
            if key == "equal":
                return self.value(value[0], parameters) == self.value(value[1], parameters)
        return expr

    def selected(self, **overrides):
        params = {k: v["default"] for k, v in self.config["parameters"].items()}
        params.update(overrides)
        return [name for name, w in self.config["workflows"].items() if self.value(w["when"], params)]

    def test_defaults_have_no_jobs_and_conflicting_request_has_one_failure_workflow(self):
        self.assertEqual(self.selected(), [])
        self.assertEqual(self.selected(build=True, target="develop"), ["invalid-request"])
        for component in ("api", "theatre-web", "studio-web", "web", "all"):
            self.assertEqual(self.selected(target="develop", component=component), ["release-" + component])
        for component in ("api", "theatre-web", "studio-web", "android", "all"):
            self.assertEqual(self.selected(build=True, **{"build-component": component}), ["build-" + component])

    def test_one_approval_after_build_and_api_health_blocks_both_web_writes(self):
        jobs = self.config["workflows"]["release-all"]["jobs"]
        graph = {next(iter(j)): next(iter(j.values())) for j in jobs}
        self.assertEqual(graph["approve-production"], {"type": "approval", "requires": ["prepare-release"]})
        self.assertEqual(graph["deploy-api"]["requires"], ["approve-production"])
        self.assertEqual(graph["deploy-theatre-web"]["requires"], ["deploy-api"])
        self.assertEqual(graph["deploy-studio-web"]["requires"], ["deploy-theatre-web"])
        self.assertEqual(graph["release-report"]["requires"], [{"deploy-api": "terminal"}, {"deploy-theatre-web": "terminal"}, {"deploy-studio-web": "terminal"}])
        for c in ("api", "theatre-web", "studio-web"):
            self.assertIn("production-" + c, graph["deploy-" + c]["serial-group"])

    def test_web_only_has_no_vps_job_and_contexts_are_separate_after_approval(self):
        for name, workflow in self.config["workflows"].items():
            for node in workflow["jobs"]:
                job, args = next(iter(node.items())) if isinstance(node, dict) else (node, {})
                if "context" in args:
                    self.assertTrue(job.startswith("deploy-"))
                    self.assertEqual(args["context"], ["production-vps" if job == "deploy-api" else "production-web"])
                if name in ("release-web", "release-theatre-web", "release-studio-web"):
                    self.assertNotEqual(job, "deploy-api")
        for job, details in self.config["jobs"].items():
            text = json.dumps(details)
            if job.startswith("deploy-"):
                self.assertNotIn("target/debug", text)
                self.assertNotIn("build-cache-restore", text)
                self.assertNotIn("install-build-trunk", text)
                self.assertIn("release-checkout", text)
                self.assertIn("teardown", text)
                self.assertIn("persist_to_workspace", text)
                steps = details["steps"]
                prepare = next(i for i, s in enumerate(steps) if isinstance(s, dict) and "release intent" in s.get("run", {}).get("command", ""))
                publish = next(i for i, s in enumerate(steps) if isinstance(s, dict) and "release promote" in s.get("run", {}).get("command", ""))
                self.assertIn("persist_to_workspace", steps[prepare + 1])
                self.assertEqual(publish, prepare + 2)
        self.assertIn("persist_to_workspace", json.dumps(self.config["jobs"]["prepare-release"]))
        self.assertNotIn("production", json.dumps(self.config["jobs"]["prepare-release"]))
        ssh = self.config["commands"]["ssh-context"]["steps"]
        self.assertIn("require-env", ssh[0])
        self.assertNotIn("apt-get", json.dumps(ssh))
        firebase = self.config["commands"]["firebase-context"]["steps"][1]["run"]["command"]
        self.assertIn("python3 - \\<<'EOF'", firebase)
