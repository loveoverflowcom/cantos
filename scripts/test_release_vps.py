"""Actual tar/config inspection plus artifact promotion with fake SSH/curl/zstd boundaries."""
import argparse
import hashlib
import io
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import time
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_pipeline as release
import release_build
from test_release_artifact import ORIGIN, SHA

# Keep tool discovery consistent with skipUnless while clearing all CI/credential inputs.
# Homebrew and other explicit tool installations need not live on os.defpath.
TOOL_PATH = os.environ.get("PATH", os.defpath)

def tar_bytes(path, files, *, host_mode=0o755):
    with tarfile.open(path, "w") as archive:
        for name, data in files.items():
            member = tarfile.TarInfo(name)
            member.size = len(data)
            member.mode = host_mode if name == "infra/vps/bin/cantos-vps" else 0o600
            archive.addfile(member, io.BytesIO(data))


def docker_archive(path, *, revision=SHA, platform="amd64", bad_layer=False):
    layer = b"fixture uncompressed layer bytes"
    config = {"architecture": platform, "os": "linux", "config": {"Labels": {
        "org.opencontainers.image.revision": revision, "io.cantos.schema-epoch": "2"}},
        "rootfs": {"diff_ids": ["sha256:" + hashlib.sha256(layer).hexdigest()]}}
    data = contract.canonical(config)
    image_id = hashlib.sha256(data).hexdigest()
    descriptor = [{"Config": image_id + ".json", "RepoTags": ["localhost/cantos-api:" + SHA], "Layers": ["layer.tar"]}]
    tar_bytes(path, {"manifest.json": contract.canonical(descriptor), image_id + ".json": data,
                     "layer.tar": b"corrupt" if bad_layer else layer})
    return {"id": "sha256:" + image_id, "source_sha": SHA, "architecture": "amd64", "os": "linux", "schema_epoch": 2}


def api_fixture(root, *, host_mode=0o755):
    output = root / "artifact"
    output.mkdir()
    image = {"id": "sha256:" + "d" * 64, "source_sha": SHA, "architecture": "amd64", "os": "linux", "schema_epoch": 2}
    contract.write_new(output / "image.json", image)
    (output / "cantos-api.tar.zst").write_bytes(b"compressed image fixture; decompressor mocked")
    tar_bytes(output / "runtime.tar", {"REVISION": (SHA + "\n").encode(),
        "infra/vps/bin/cantos-vps": b"#!/bin/sh\nexit 0\n", "infra/edge/Caddyfile": b"fixture",
        "services/cantos-server/schema/legacy-epoch-1-manifest.json": b"{}"}, host_mode=host_mode)
    config = {"component": "api", "api_origin": None, "profile": "release", "platform": "linux/amd64"}
    manifest = {"schema_version": 2, "repository": contract.REPOSITORY, "target": "api",
        "source_sha": SHA, "checkout_sha": SHA, "config_sha": SHA, "deployed": False,
        "created_at": time.time(), "api_origin": None, "configuration": config,
        "configuration_sha256": contract.fingerprint(config),
        "artifacts": [contract.file_record(output / name, output) for name in sorted(contract.EXPECTED["api"])]}
    path = output / "manifest.json"
    contract.write_new(path, manifest)
    return argparse.Namespace(release_manifest=path, release_digest=contract.file_record(path, output)["sha256"],
        release_sha=SHA, release_request=None, local_artifact=True, api_origin=ORIGIN,
        host="root@vps.example.org", dry_run=False)


@patch.dict(os.environ, {"PATH": TOOL_PATH}, clear=True)
class VpsTests(unittest.TestCase):
    def test_committed_host_entrypoint_and_extracted_release_are_executable(self):
        repository = Path(__file__).resolve().parents[1]
        entrypoint = "infra/vps/bin/cantos-vps"
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repository, text=True).strip()
        tree = subprocess.check_output(["git", "ls-tree", revision, "--", entrypoint],
                                       cwd=repository, text=True)
        self.assertEqual(tree.split()[0], "100755", tree)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            context = root / "context"
            context.mkdir()
            source = root / "source.tar"
            subprocess.run(["git", "archive", "--format=tar", "-o", str(source), revision,
                            "infra/vps", "infra/edge/Caddyfile",
                            "services/cantos-server/schema/legacy-epoch-1-manifest.json"],
                           cwd=repository, check=True)
            subprocess.run(["tar", "-xf", str(source), "-C", str(context)], check=True)
            self.assertTrue(os.access(context / entrypoint, os.X_OK))
            (context / "REVISION").write_text(revision + "\n")
            runtime = root / "runtime.tar"
            release_build.runtime_archive(context, runtime)
            contract.extract(runtime, root / "tree", runtime=True,
                             expected=contract.file_record(runtime, root))
            self.assertTrue(os.access(root / "tree" / entrypoint, os.X_OK))
            host_tree = root / "host-tree"
            host_tree.mkdir()
            subprocess.run(["tar", "-xf", str(runtime), "-C", str(host_tree)], check=True)
            self.assertTrue(os.access(host_tree / entrypoint, os.X_OK))
            self.assertEqual((root / "tree/REVISION").read_text().strip(), revision)
            self.assertEqual((root / "tree" / entrypoint).read_bytes(), (context / entrypoint).read_bytes())
            (context / entrypoint).chmod(0o644)
            with self.assertRaisesRegex(ValueError, "runtime entrypoint must be executable"):
                release_build.runtime_archive(context, root / "non-executable.tar")

    @patch.object(release, "inspect_image")
    def test_mode_100644_is_rejected_before_extraction_and_any_remote_command(self, inspect):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            # The manifest hashes the actual 0644 tar: this is a mode failure, not corruption.
            args = api_fixture(root, host_mode=0o644)
            with self.assertRaisesRegex(ValueError, "runtime entrypoint must be executable"):
                contract.extract(args.release_manifest.parent / "runtime.tar", root / "tree", runtime=True)
            self.assertFalse((root / "tree").exists())
            with patch.object(release, "run") as run:
                with self.assertRaisesRegex(ValueError, "retired"):
                    release.deploy_api(root, args)
                run.assert_not_called()

    @unittest.skipUnless(shutil.which("zstd"), "installed zstd required for actual decoder evidence")
    def test_real_zstd_frame_decodes_and_truncation_fails_before_any_ssh(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "source.tar"
            image = docker_archive(archive)
            compressed = root / "image.tar.zst"
            subprocess.run(["zstd", "-q", str(archive), "-o", str(compressed)], check=True)
            destination = root / "valid"
            destination.mkdir()
            release.inspect_image(root, compressed, image, SHA, destination)
            compressed.write_bytes(compressed.read_bytes()[:12])
            destination = root / "truncated"
            destination.mkdir()
            with self.assertRaisesRegex(ValueError, "truncated/corrupt"):
                release.inspect_image(root, compressed, image, SHA, destination)

    def test_docker_archive_id_actual_layer_source_platform_and_schema_are_checked(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "image.tar"
            image = docker_archive(path)
            release.inspect_docker_archive(path, image, SHA)
            with self.assertRaisesRegex(ValueError, "image ID"):
                release.inspect_docker_archive(path, {"id": "sha256:" + "0" * 64}, SHA)
            for values, reason in [({"revision": "c" * 40}, "source/platform/schema"),
                                   ({"platform": "arm64"}, "source/platform/schema"),
                                   ({"bad_layer": True}, "corrupt image layer")]:
                with self.subTest(values=values):
                    image = docker_archive(path, **values)
                    with self.assertRaisesRegex(ValueError, reason):
                        release.inspect_docker_archive(path, image, SHA)

    def test_retired_executor_refuses_before_any_command_or_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for dry_run in (False, True):
                with patch.object(release, "run") as run:
                    with self.assertRaisesRegex(ValueError, "retired"):
                        release.deploy_api(root, argparse.Namespace(dry_run=dry_run))
                    run.assert_not_called()
            self.assertEqual(list(root.iterdir()), [])


@unittest.skipUnless(shutil.which("flock"), "Linux host flock required for actual lock evidence")
class HostPromotionTests(unittest.TestCase):
    def host(self, *, wrong_id=False, pending=False, schema_failure=False):
        source = (Path(__file__).resolve().parents[1] / "infra/vps/bin/cantos-vps").read_text().removesuffix('\nmain "$@"\n')
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tree = root / "incoming" / SHA / ("e" * 32) / "tree"
            tree.mkdir(parents=True)
            (tree / "REVISION").write_text(SHA + "\n")
            (root / "state").mkdir()
            if pending:
                (root / "state/epoch2-cutover.pending").write_text("retained journal")
            source = source.replace("readonly LOCK=/run/lock/cantos-vps.lock", "readonly LOCK=" + str(root / "lock"))
            doubles = r'''
event() { printf '%s\n' "$*" >>"$CANTOS_TEST_EVENTS"; }
require_root() { :; }
uname() { printf 'Linux x86_64\n'; }
sha256sum() { event checksum; }
zstd() { event decompress; printf 'fixture image'; }
podman() {
  if [[ $1 == load ]]; then cat >/dev/null; event load; return; fi
  case "$4" in
    '{{.Id}}') printf '%s\n' "$TEST_IMAGE_ID" ;;
    '{{.Os}}/{{.Architecture}}') printf 'linux/amd64\n' ;;
    *org.opencontainers.image.revision*) printf '%s\n' "$TEST_REVISION" ;;
    *schema-epoch*) printf '2\n' ;;
    *) return 1 ;;
  esac
}
locked() {
  [[ $CANTOS_VPS_LOCK_OWNER == "$BASHPID" ]] || return 91
  if flock -n "$LOCK" true; then return 92; fi
  event "$1-locked"
}
install_bundle() { locked install; }
deploy() { locked deploy; }
ensure_private_media_storage() { locked storage-prepare; }
validate_runtime_browser_configuration() { event browser-check; }
unit_active() { return 0; }
preflight_schema() { event schema-check; return "$TEST_SCHEMA_STATUS"; }
'''
            image_id = "sha256:" + "d" * 64
            body = '\nmain promote "$TEST_REVISION" "$TEST_TREE" "$TEST_EXPECTED_ID"\n'
            env = dict(os.environ, CANTOS_VPS_ROOT=str(root), CANTOS_TEST_EVENTS=str(root / "events"),
                TEST_IMAGE_ID="c" * 64 if wrong_id else "d" * 64,
                TEST_EXPECTED_ID=image_id, TEST_REVISION=SHA, TEST_TREE=str(tree))
            env["TEST_SCHEMA_STATUS"] = "65" if schema_failure else "0"
            result = subprocess.run(["bash"], input=source + doubles + body, text=True, capture_output=True, env=env)
            return result, (root / "events").read_text().splitlines() if (root / "events").exists() else []

    def test_actual_host_protocol_locks_image_load_through_runtime_and_deploy(self):
        result, events = self.host()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(events, ["checksum", "decompress", "load", "browser-check", "schema-check", "storage-prepare-locked", "install-locked", "deploy-locked"])

    def test_wrong_loaded_image_and_pending_cutover_refuse_before_runtime_install(self):
        result, events = self.host(wrong_id=True)
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("image ID mismatch", result.stderr)
        self.assertNotIn("install-locked", events)
        result, events = self.host(pending=True)
        self.assertEqual(result.returncode, 75, result.stderr)
        self.assertEqual(events, [])
        result, events = self.host(schema_failure=True)
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("schema-check", events)
        self.assertNotIn("install-locked", events)
