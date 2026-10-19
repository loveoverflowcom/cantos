"""Independent malformed-byte/provenance oracles for the actual release validator."""
import copy
import hashlib
import io
import os
from pathlib import Path
import tarfile
import tempfile
import time
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_build


SHA = "a" * 40
ORIGIN = "https://api.example.org"
ENV = {"CIRCLECI": "true", "CIRCLE_SHA1": "b" * 40, "CIRCLE_BRANCH": "develop",
       "CANTOS_CONFIG_SHA": "b" * 40, "CANTOS_CONFIG_REF": "refs/heads/develop",
       "CANTOS_CONFIG_REPOSITORY": contract.REPOSITORY,
       "CANTOS_PIPELINE_ID": "pipeline-1", "CANTOS_PIPELINE_NUMBER": "12",
       "CIRCLE_WORKFLOW_ID": "workflow-1", "CIRCLE_WORKFLOW_JOB_ID": "prepare-job", "CIRCLE_BUILD_NUM": "321"}


def request():
    return {"schema_version": 1, "repository": contract.REPOSITORY,
            "config_sha": "b" * 40, "initial_checkout_sha": "b" * 40, "source_sha": SHA, "api_origin": ORIGIN,
            "producer": contract.ci_identity(ENV), "created_at": time.time(),
            "build_only": False, "components": ["theatre-web"]}


def fixture(root, req=None):
    output = root / "artifact"
    output.mkdir()
    with tarfile.open(output / "theatre-web.tar.gz", "w:gz") as archive:
        for name, data in [("index.html", b"<html>fixture</html>"), ("app.wasm", b"wasm fixture")]:
            member = tarfile.TarInfo(name)
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))
    config = {"component": "theatre-web", "api_origin": ORIGIN, "profile": "release"}
    manifest = {"schema_version": 2, "target": "theatre-web", "source_sha": SHA,
                "repository": contract.REPOSITORY, "checkout_sha": SHA, "config_sha": "b" * 40,
                "initial_checkout_sha": "b" * 40,
                "deployed": False, "created_at": time.time(), "api_origin": ORIGIN,
                "configuration": config, "configuration_sha256": contract.fingerprint(config),
                "artifacts": [contract.file_record(output / "theatre-web.tar.gz", output)],
                "producer": req["producer"] if req else None,
                "request_sha256": contract.fingerprint(req) if req else None}
    path = output / "manifest.json"
    contract.write_new(path, manifest)
    return path, manifest


@patch.dict(os.environ, {}, clear=True)
class ArtifactTests(unittest.TestCase):
    def test_podman_bare_config_ids_are_normalized_without_changing_the_digest(self):
        self.assertEqual(contract.image_id("d" * 64), "sha256:" + "d" * 64)
        self.assertEqual(contract.image_id("sha256:" + "d" * 64), "sha256:" + "d" * 64)
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            contract.image_id("latest")

    def validate(self, path, **changes):
        args = {"expected_digest": contract.file_record(path, path.parent)["sha256"],
                "component": "theatre-web", "source_sha": SHA, "origin": ORIGIN, "local": True}
        args.update(changes)
        return contract.validate(path, **args)

    def test_valid_explicit_local_bytes_are_accepted_and_hashes_do_not_claim_ci_provenance(self):
        with tempfile.TemporaryDirectory() as temporary:
            path, _ = fixture(Path(temporary))
            self.assertEqual(self.validate(path)["source_sha"], SHA)
            with self.assertRaisesRegex(ValueError, "acknowledgement"):
                self.validate(path, local=False)
            with patch.dict(os.environ, {"CIRCLECI": "true"}):
                with self.assertRaisesRegex(ValueError, "acknowledgement"):
                    self.validate(path)
            pending = path.rename(path.parent / ".pending-manifest.json")
            with self.assertRaisesRegex(ValueError, "final manifest"):
                self.validate(pending)

    def test_wrong_identity_origin_manifest_digest_and_missing_or_corrupt_bytes_are_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            path, _ = fixture(Path(temporary))
            for kwargs, reason in [({"component": "studio-web"}, "wrong component"),
                                    ({"source_sha": "c" * 40}, "wrong component"),
                                    ({"origin": "https://other.example.org"}, "wrong API"),
                                    ({"expected_digest": "0" * 64}, "manifest digest")]:
                with self.subTest(kwargs=kwargs), self.assertRaisesRegex(ValueError, reason):
                    self.validate(path, **kwargs)
            bundle = path.parent / "theatre-web.tar.gz"
            bundle.write_bytes(b"truncated")
            with self.assertRaisesRegex(ValueError, "bytes/hash"):
                self.validate(path)
            bundle.unlink()
            with self.assertRaisesRegex(ValueError, "unexpected release files"):
                self.validate(path)

    def test_manifest_v1_stale_configuration_extra_files_and_duplicate_records_are_refused(self):
        mutations = [(lambda m: m.update(schema_version=1), "v2"),
                     (lambda m: m.update(created_at=time.time() - contract.MAX_AGE - 1), "stale"),
                     (lambda m: m["configuration"].update(api_origin="https://foreign.example.org"), "configuration digest"),
                     (lambda m: m["artifacts"].append(copy.deepcopy(m["artifacts"][0])), "artifact count")]
        for mutation, reason in mutations:
            with self.subTest(reason=reason), tempfile.TemporaryDirectory() as temporary:
                path, manifest = fixture(Path(temporary))
                mutation(manifest)
                path.write_bytes(contract.canonical(manifest))
                with self.assertRaisesRegex(ValueError, reason):
                    self.validate(path)
        with tempfile.TemporaryDirectory() as temporary:
            path, _ = fixture(Path(temporary))
            (path.parent / "foreign.txt").write_text("unreviewed")
            with self.assertRaisesRegex(ValueError, "unexpected release files"):
                self.validate(path)

    def test_same_workflow_requires_actual_config_revision_and_foreign_run_is_refused(self):
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, ENV, clear=True):
            root = Path(temporary)
            req = request()
            path, _ = fixture(root, req)
            rp = root / "request.json"
            contract.write_new(rp, req)
            self.assertEqual(self.validate(path, local=False, request_path=rp)["producer"]["job_id"], "prepare-job")
            for env, reason in [({"CIRCLE_SHA1": SHA}, "config revision"),
                                ({"CANTOS_CONFIG_SHA": "c" * 40}, "config revision"),
                                ({"CIRCLE_WORKFLOW_ID": "foreign"}, "foreign pipeline"),
                                ({"CANTOS_PIPELINE_ID": "foreign"}, "foreign pipeline")]:
                with self.subTest(env=env), patch.dict(os.environ, env):
                    with self.assertRaisesRegex(ValueError, reason):
                        self.validate(path, local=False, request_path=rp)
            with patch.dict(os.environ, {"CANTOS_CONFIG_SHA": ""}):
                with self.assertRaisesRegex(ValueError, "full source SHA"):
                    self.validate(path, local=False, request_path=rp)
            req["producer"]["job_id"] = "foreign-prepare-job"
            rp.write_bytes(contract.canonical(req))
            with self.assertRaisesRegex(ValueError, "foreign-run"):
                self.validate(path, local=False, request_path=rp)

    def test_config_sha_can_differ_from_release_sha_without_disabling_clean_source_guard(self):
        req = request()
        with patch.dict(os.environ, ENV, clear=True):
            with patch.object(release_build, "run", side_effect=[SHA, "", "", ""]) as run:
                self.assertEqual(release_build.source_revision(Path("."), req), SHA)
                self.assertEqual(run.call_args_list[-1].args[1],
                                 ["git", "merge-base", "--is-ancestor", SHA, "refs/remotes/origin/develop"])
            with patch.object(release_build, "run", side_effect=[SHA, " M dirty"]):
                with self.assertRaisesRegex(ValueError, "clean"):
                    release_build.source_revision(Path("."), req)
            with patch.object(release_build, "run", side_effect=["c" * 40, ""]):
                with self.assertRaisesRegex(ValueError, "differs from resolved"):
                    release_build.source_revision(Path("."), req)

    def test_archive_escape_links_duplicates_devices_prefix_collisions_and_limits_fail_before_extraction(self):
        cases = [("/absolute.html", tarfile.REGTYPE), ("../escape.html", tarfile.REGTYPE),
                 ("a/../../escape.html", tarfile.REGTYPE), ("link.html", tarfile.SYMTYPE),
                 ("hard.html", tarfile.LNKTYPE), ("fifo.html", tarfile.FIFOTYPE),
                 ("a\\escape.html", tarfile.REGTYPE), (".env", tarfile.REGTYPE)]
        for name, kind in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                archive = root / "bad.tar"
                with tarfile.open(archive, "w") as handle:
                    entry = tarfile.TarInfo(name)
                    entry.type = kind
                    handle.addfile(entry)
                with self.assertRaises(ValueError):
                    contract.extract(archive, root / "dist", web=True)
                self.assertFalse((root / "dist").exists())
        for names in [["index.html", "index.html"], ["index.html", "index.html/a.js"]]:
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                with tarfile.open(root / "bad.tar", "w") as handle:
                    for name in names:
                        handle.addfile(tarfile.TarInfo(name))
                with self.assertRaisesRegex(ValueError, "duplicate|prefix"):
                    contract.extract(root / "bad.tar", root / "dist", web=True)
                self.assertFalse((root / "dist").exists())
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path, _ = fixture(root)
            with patch.object(contract, "MAX_ENTRIES", 1):
                with self.assertRaisesRegex(ValueError, "limits"):
                    self.validate(path)

    def test_origin_is_public_validated_input_and_configuration_identity_includes_it(self):
        root = Path(__file__).resolve().parents[1]
        first = contract.configuration(root, "theatre-web", ORIGIN)
        second = contract.configuration(root, "theatre-web", "https://other.example.org")
        self.assertNotEqual(contract.fingerprint(first), contract.fingerprint(second))
        self.assertIn("infra/firebase/hosting/firebase.json", first["policy"])
        for invalid in [None, "", "http://api.example.org", "https://api.example.org/", "https://u:p@api.example.org",
                        "https://api.example.org:443", "https://api.example.org?", "https://api.example.org#",
                        "https://api.example.org\n", "https://127.1", "https://0x7f000001"]:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                contract.api_origin(invalid)
        self.assertEqual(contract.api_origin("https://127.0.0.1:8443"), "https://127.0.0.1:8443")

    def test_archive_replaced_after_validation_is_refused_before_any_dist_write(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path, manifest = fixture(root)
            self.validate(path)
            archive = path.parent / "theatre-web.tar.gz"
            archive.write_bytes(b"replaced after validation")
            with self.assertRaisesRegex(ValueError, "changed before staging"):
                contract.extract(archive, root / "dist", web=True, expected=manifest["artifacts"][0])
            self.assertFalse((root / "dist").exists())
