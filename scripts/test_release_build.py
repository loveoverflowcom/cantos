"""Source binding, failure containment and artifact inventory regressions; no build/provider."""
import hashlib
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import release_build as release
import release_artifact as contract


class ReleaseBuildTests(unittest.TestCase):
    def test_android_build_uses_the_declared_origin(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(release, "run", side_effect=RuntimeError("stop before build")) as run:
                with self.assertRaisesRegex(RuntimeError, "stop before build"):
                    release.build(root, "android", "a" * 40, root, "https://catalog.example")
            self.assertEqual(run.call_args.args[1][-2:], ["--api-origin", "https://catalog.example"])

    def test_android_package_preserves_the_declared_origin(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(release, "source_revision", return_value="a" * 40), \
                 patch.object(contract, "configuration", side_effect=RuntimeError("stop at config")) as config:
                with self.assertRaisesRegex(RuntimeError, "stop at config"):
                    release.package(root, "android", origin="https://catalog.example")
            self.assertEqual(config.call_args.args[-1], "https://catalog.example")

    def test_android_staging_refuses_foreign_configuration_and_snapshot_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            report = root / release.MOBILE / "androidApp/build/reports/release-boundary/release-evidence.json"
            report.parent.mkdir(parents=True)
            for origin, issuer, apk, snapshot, message in (
                ("https://other.example", "", "androidApp-release-unsigned.apk", "artifacts/android-release/run", "configuration differs"),
                ("https://catalog.example", "unexpected issuer", "androidApp-release-unsigned.apk", "artifacts/android-release/run", "configuration differs"),
                ("https://catalog.example", "", "androidApp-release.apk", "artifacts/android-release/run", "configuration differs"),
                ("https://catalog.example", "", "androidApp-release-unsigned.apk", "../../outside", "unsafe archive path"),
                ("https://catalog.example", "", "androidApp-release-unsigned.apk", "artifacts/other/run", "invalid Android snapshot"),
            ):
                with self.subTest(origin=origin, apk=apk, snapshot=snapshot):
                    report.write_text(json.dumps({"configuration": {"api_origin": origin, "native_issuer": issuer},
                        "artifacts": {"optimized": {"apk": {"path": apk}}}, "snapshot_directory": snapshot}))
                    with patch.object(release, "run"), self.assertRaisesRegex(ValueError, message):
                        release.build(root, "android", "c" * 40, root / "out", "https://catalog.example")

    def test_android_staging_binds_the_immutable_record_before_copying(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            report = root / release.MOBILE / "androidApp/build/reports/release-boundary/release-evidence.json"
            report.parent.mkdir(parents=True)
            snapshot = root / "artifacts/android-release/run"
            snapshot.mkdir(parents=True)
            evidence = {"configuration": {"api_origin": "https://catalog.example"},
                        "artifacts": {"optimized": {"apk": {"path": "androidApp-release-unsigned.apk"}}},
                        "snapshot_directory": "artifacts/android-release/run"}
            report.write_text(json.dumps(evidence))
            # Another completed build has the same source but a different configuration.
            evidence["configuration"]["native_issuer"] = "foreign issuer"
            (snapshot / "release-evidence.json").write_text(json.dumps(evidence))
            with patch.object(release, "run"), self.assertRaisesRegex(ValueError, "snapshot record differs"):
                release.build(root, "android", "c" * 40, root / "out", "https://catalog.example")
            self.assertFalse((root / "out").exists())

    def test_hash_and_size_are_from_actual_bytes_and_symlinks_are_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            artifact = output / "app.apk"
            artifact.write_bytes(b"actual apk bytes")
            self.assertEqual(release.file_record(artifact, output), {
                "path": "app.apk", "bytes": 16,
                "sha256": hashlib.sha256(b"actual apk bytes").hexdigest()})
            link = output / "other.apk"
            link.symlink_to(artifact)
            with self.assertRaisesRegex(ValueError, "symlink"):
                release.file_record(link, output)

    def test_web_archive_contains_only_bundle_and_rejects_secret_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            dist = output / "dist"
            dist.mkdir()
            (dist / "index.html").write_text("<html>Vietnamese: Truyện</html>")
            (dist / "app.wasm").write_bytes(b"wasm")
            release.web_archive(dist, output / "bundle.tar.gz")
            with tarfile.open(output / "bundle.tar.gz") as archive:
                self.assertEqual(archive.getnames(), ["app.wasm", "index.html"])
            (dist / ".env").write_text("secret fixture")
            with self.assertRaisesRegex(ValueError, "unexpected Web asset"):
                release.web_archive(dist, output / "invalid.tar.gz")

    @patch.dict("os.environ", {}, clear=True)
    def test_dirty_source_or_circle_sha_mismatch_is_refused(self):
        with patch.object(release, "run", side_effect=["a" * 40, " M changed"]):
            with self.assertRaisesRegex(ValueError, "clean"):
                release.source_revision(Path("."))
        with patch.dict("os.environ", {"CIRCLE_SHA1": "b" * 40}):
            with patch.object(release, "run", side_effect=["a" * 40, ""]):
                with self.assertRaisesRegex(ValueError, "differs"):
                    release.source_revision(Path("."))

    @patch.object(contract, "configuration", return_value={"component": "android", "api_origin": "https://api.cantos.cc", "profile": "release"})
    @patch.object(release, "source_revision", side_effect=["a" * 40, "b" * 40])
    @patch.object(release, "build", return_value={"name": "1"})
    def test_changed_source_never_gets_an_accepted_manifest(self, build, revision, config):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, "changed during"):
                release.package(root, "api")
            self.assertEqual(list(root.rglob("manifest.json")), [])

    @patch.object(contract, "configuration", return_value={"component": "android", "api_origin": "https://api.cantos.cc", "profile": "release"})
    @patch.object(release, "source_revision", return_value="a" * 40)
    def test_failed_attempts_have_no_manifest_and_reruns_use_fresh_staging(self, revision, config):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(release, "build", side_effect=RuntimeError("compiler failed")):
                with self.assertRaisesRegex(RuntimeError, "compiler failed"):
                    release.package(root, "android")
            self.assertEqual(list(root.rglob("manifest.json")), [])
            with patch.object(release, "build", side_effect=RuntimeError("second attempt")):
                with self.assertRaisesRegex(RuntimeError, "second attempt"):
                    release.package(root, "android")
            attempts = root / "artifacts/release-build/android" / ("a" * 40)
            self.assertEqual(len(list(attempts.glob("*/*"))), 2)

    @patch.dict("os.environ", {"SECRET_TOKEN": "must-not-appear"}, clear=True)
    @patch.object(contract, "configuration", return_value={"component": "android", "api_origin": "https://api.cantos.cc", "profile": "release"})
    @patch.object(release, "source_revision", return_value="a" * 40)
    @patch.object(release, "run", return_value="rustc 1.98.1")
    def test_manifest_binds_artifact_bytes_and_does_not_dump_environment(self, run, revision, config):
        def fake_build(root, target, sha, output, origin):
            for name in contract.EXPECTED[target]:
                (output / name).write_bytes(b"release bytes")
            return {"name": "0.1.0", "code": 1}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(release, "build", side_effect=fake_build), patch.object(contract, "validate_android_outputs"):
                release.package(root, "android")
            text = next(root.rglob("manifest.json")).read_text()
            manifest = json.loads(text)
            self.assertEqual(manifest["application_id"], "com.cantos.theatre")
            self.assertEqual(manifest["signing"], "unsigned")
            self.assertFalse(manifest["deployed"])
            self.assertEqual(manifest["api_origin"], "https://api.cantos.cc")
            self.assertNotIn("must-not-appear", text)
            self.assertEqual(manifest["artifacts"][0]["sha256"], hashlib.sha256(b"release bytes").hexdigest())

    def test_android_version_comes_from_production_host(self):
        root = Path(__file__).resolve().parents[1]
        self.assertEqual(release.android_version(root), {"name": "0.1.0", "code": 1})

    @patch.object(release, "run")
    def test_dry_run_writes_nothing_and_executes_nothing(self, run):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            release.package(root, "api", dry_run=True)
            self.assertEqual(list(root.iterdir()), [])
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
