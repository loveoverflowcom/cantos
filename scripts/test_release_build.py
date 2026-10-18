"""Source binding, failure containment and artifact inventory regressions; no build/provider."""
import hashlib
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import release_build as release


class ReleaseBuildTests(unittest.TestCase):
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
            with self.assertRaisesRegex(ValueError, "regular file"):
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

    @patch.object(release, "source_revision", side_effect=["a" * 40, "b" * 40])
    @patch.object(release, "build", return_value={"name": "1"})
    def test_changed_source_never_gets_an_accepted_manifest(self, build, revision):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, "changed during"):
                release.package(root, "api")
            self.assertEqual(list(root.rglob("manifest.json")), [])

    @patch.object(release, "source_revision", return_value="a" * 40)
    def test_build_failure_is_not_success_and_rerun_cannot_overwrite(self, revision):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(release, "build", side_effect=RuntimeError("compiler failed")):
                with self.assertRaisesRegex(RuntimeError, "compiler failed"):
                    release.package(root, "android")
            self.assertEqual(list(root.rglob("manifest.json")), [])
            with patch.object(release, "build") as build:
                with self.assertRaises(FileExistsError):
                    release.package(root, "android")
                build.assert_not_called()

    @patch.dict("os.environ", {"SECRET_TOKEN": "must-not-appear"}, clear=True)
    @patch.object(release, "source_revision", return_value="a" * 40)
    @patch.object(release, "run", return_value="rustc 1.98.1")
    def test_manifest_binds_artifact_bytes_and_does_not_dump_environment(self, run, revision):
        def fake_build(root, target, sha, output):
            (output / "release.apk").write_bytes(b"release bytes")
            return {"name": "0.1.0", "code": 1}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(release, "build", side_effect=fake_build):
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
