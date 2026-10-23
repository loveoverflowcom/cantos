"""Release refusals and recovery use production owners with finite command doubles."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_oci as oci
import release_oci_build as build
import release_pipeline as release
from test_release_oci_build import Shell, native_amd64_host


class Retirement(unittest.TestCase):
    def test_legacy_aggregate_selection_refuses_before_source_or_network(self):
        for component in ("api", "all"):
            for build_only in (False, True):
                with self.subTest(component=component, build_only=build_only):
                    with patch.object(release.release_build, "source_revision") as source:
                        with self.assertRaisesRegex(ValueError, "retired"):
                            release.resolve(Path.cwd(), None if build_only else "develop",
                                            component, "https://api.example.org", build_only)
                        source.assert_not_called()

    def test_legacy_python_deployer_refuses_even_dry_run_before_validation_or_io(self):
        for dry in (False, True):
            with patch.object(release, "run") as run, patch.object(release, "validated") as validate:
                with self.assertRaisesRegex(ValueError, "retired"):
                    release.deploy_api(Path.cwd(), type("Args", (), {"dry_run": dry})())
                run.assert_not_called()
                validate.assert_not_called()

    def test_no_circleci_workflow_receives_vps_credentials_or_calls_the_old_job(self):
        config = json.loads((Path(__file__).resolve().parents[1] / ".circleci/config.yml").read_text())
        for name in ("release-api", "release-all", "build-api", "build-all"):
            self.assertEqual(config["workflows"][name]["jobs"], ["reject-retired-release"])
        for workflow in config["workflows"].values():
            self.assertNotIn("production-vps", json.dumps(workflow))
            self.assertNotIn('"deploy-api"', json.dumps(workflow))


class PublishRecovery(Shell):
    def auth(self):
        path = Path(self.temporary.name) / "private-auth.json"
        path.write_text('{"auths":{"ghcr.io":{"auth":"dXNlcjpmaXh0dXJl"}}}')
        path.chmod(0o600)
        return path

    def journal(self):
        paths = list((self.root / "out").rglob("publish-attempt.json"))
        self.assertEqual(len(paths), 1)
        return contract.read_json(paths[0])

    def test_authfile_is_refused_before_any_build_or_push(self):
        auth = self.auth()
        auth.chmod(0o644)
        with patch.object(build, "build_pair") as pair, native_amd64_host():
            with self.assertRaisesRegex(Exception, "AUTHFILE_PERMISSIONS"):
                build.produce(self.root, self.args(True, auth))
            pair.assert_not_called()
        self.assertFalse((self.root / "out").exists())

    def test_second_push_timeout_preserves_first_digest_and_unknown_without_retry(self):
        fake = self.fake("sha256:" + "9" * 64)
        def interrupted(root, argv, **kwargs):
            if argv[:2] == ["podman", "push"] and "cantos-identity:" in argv[-1]:
                self.calls.append(("push", argv[-1]))
                raise subprocess.TimeoutExpired(argv, 1, stderr=b"private-provider-text")
            return fake(root, argv, **kwargs)
        with patch.object(build, "sh", interrupted), native_amd64_host():
            with self.assertRaises(subprocess.TimeoutExpired):
                build.produce(self.root, self.args(True, self.auth()))
        record = self.journal()
        self.assertEqual(record["images"]["cantos-server"]["state"], "confirmed")
        self.assertEqual(record["images"]["cantos-server"]["digest"], "sha256:" + "9" * 64)
        self.assertEqual(record["images"]["cantos-identity"]["state"], "unknown")
        self.assertEqual(record["registry_readback"], "NOT_RUN")
        self.assertEqual(len([c for c in self.calls if c[0] == "push"]), 2)
        self.assertFalse(list((self.root / "out").rglob("manifest.json")))
        self.assertNotIn("private-provider-text", json.dumps(record))
        self.assertNotIn("private-auth", json.dumps(record))

    def test_readback_failure_retains_candidate_and_both_confirmed_pushes(self):
        with patch.object(build, "sh", self.fake("sha256:" + "9" * 64)), native_amd64_host(), \
                patch("release_oci_registry.observe", side_effect=ValueError("readback failed")):
            with self.assertRaisesRegex(ValueError, "readback failed"):
                build.produce(self.root, self.args(True, self.auth()))
        self.assertEqual({v["state"] for v in self.journal()["images"].values()}, {"confirmed"})
        self.assertEqual(self.journal()["registry_readback"], "unknown")
        self.assertEqual(len(list((self.root / "out").rglob("manifest.json"))), 1)

    def test_build_child_environment_drops_provider_tokens_and_ambient_auth(self):
        ambient = {"PATH": os.defpath, "HOME": "/fixture", "GITHUB_TOKEN": "private",
                   "CLOUDFLARE_API_TOKEN": "private", "REGISTRY_AUTH_FILE": "/private/auth"}
        completed = subprocess.CompletedProcess([], 0, b"ok", b"")
        with patch.dict(os.environ, ambient, clear=True), patch.object(build.subprocess, "run", return_value=completed) as run:
            self.assertEqual(build.sh(self.root, ["podman", "build"]), "ok")
        child = run.call_args.kwargs["env"]
        self.assertEqual(child["PATH"], os.defpath)
        self.assertNotIn("GITHUB_TOKEN", child)
        self.assertNotIn("CLOUDFLARE_API_TOKEN", child)
        self.assertNotIn("REGISTRY_AUTH_FILE", child)


if __name__ == "__main__":
    unittest.main()
