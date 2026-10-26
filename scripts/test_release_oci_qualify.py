"""Qualification core/file regressions; these tests never establish Linux image acceptance."""

import copy
import gzip
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_oci as oci
import release_oci_build as build
import release_oci_qualify as qualify
from test_release_oci_build import git, images, repository


def layer(files):
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w") as archive:
        for name, data in files.items():
            member = tarfile.TarInfo(name)
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))
    return buffer.getvalue()


class QualificationContract(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.work = Path(self.temporary.name).resolve()
        self.root = self.work / "repo"
        self.root.mkdir()
        repository(self.root)
        for name in qualify.FIXTURE_FILES:
            (self.root / name).write_text("# fixture-bound implementation\n")
        git(self.root, "add", ".")
        git(self.root, "-c", "user.name=Fixture", "-c", "user.email=f@example.invalid", "commit", "--quiet", "-m", "fixture driver")
        self.source = git(self.root, "rev-parse", "HEAD")
        self.configuration = build.configuration_identity(self.root, self.source, "linux/amd64")
        self.manifest, bundle, evidence, records = build.candidate(
            self.source, self.configuration, "linux/amd64", {"kind": "local", "build_id": "test"},
            build.compatibility(self.root, self.source), images(), build.host_members(self.root, self.source))
        self.candidate = self.work / "candidate"
        self.digest = build.write_candidate(self.candidate, self.manifest, bundle, evidence, records)
        self.args = SimpleNamespace(manifest=self.candidate / "manifest.json", manifest_digest=self.digest,
                                    source_sha=self.source, configuration_digest=self.configuration,
                                    platform="linux/amd64", authfile=None, output=self.work / "qualified")

    def test_unadmitted_candidate_can_be_tested_but_existing_admission_gate_still_refuses(self):
        manifest, _ = qualify.validate_input(self.root, self.args)
        self.assertEqual(manifest, self.manifest)
        with self.assertRaisesRegex(ValueError, "did not PASS"):
            oci.verify_evidence(self.candidate / "evidence.json", manifest)
        with self.assertRaisesRegex(ValueError, "admission"):
            oci.manifest_plan(manifest, {}, manifest_digest=self.digest, source_sha=self.source,
                              configuration_digest=self.configuration, platform="linux/amd64")

    def test_all_input_identity_mismatches_refuse_before_any_runtime(self):
        for field, value in (("manifest_digest", "f" * 64), ("source_sha", "e" * 40),
                             ("configuration_digest", "d" * 64), ("platform", "linux/arm64")):
            with self.subTest(field=field):
                args = copy.copy(self.args)
                setattr(args, field, value)
                with patch.object(qualify, "Fixture") as runtime:
                    with self.assertRaises(ValueError):
                        qualify.qualify(self.root, args)
                    runtime.assert_not_called()

    def test_dirty_or_different_qualifier_source_cannot_attest_the_image(self):
        (self.root / qualify.FIXTURE_FILES[0]).write_text("# changed after image build\n")
        with self.assertRaisesRegex(ValueError, "SOURCE_DIRTY"):
            qualify.validate_input(self.root, self.args)

    def test_mac_emulation_and_nonroot_do_not_satisfy_native_host(self):
        for system, machine, uid in (("Darwin", "arm64", 0), ("Linux", "x86_64", 501)):
            with self.assertRaisesRegex(ValueError, "REQUIRED"):
                qualify.native_host(SimpleNamespace(system=lambda: system, machine=lambda: machine), uid)
        self.assertEqual(qualify.native_host(SimpleNamespace(system=lambda: "Linux", machine=lambda: "x86_64"), 0),
                         "linux/amd64")

    def test_unavailable_runtime_writes_only_not_run_and_no_integration_claim(self):
        original = (self.candidate / "manifest.json").read_bytes()
        with patch.object(qualify, "native_host", side_effect=ValueError("LINUX_NATIVE_REQUIRED")), \
                patch.object(qualify, "Fixture") as runtime:
            result = qualify.qualify(self.root, self.args)
            runtime.assert_not_called()
        self.assertEqual(set(result["gates"].values()), {"NOT_RUN"})
        self.assertFalse(result["release_eligible"])
        for path in (self.args.output / "gate-records").glob("*.json"):
            record = json.loads(path.read_bytes())
            self.assertNotIn("evidence_level", record)
            self.assertEqual(record["images"], self.manifest["images"])
        self.assertEqual(original, (self.candidate / "manifest.json").read_bytes())
        with self.assertRaises(FileExistsError):
            qualify.qualify(self.root, self.args)

    def test_fake_input_pass_records_are_not_reused_and_runtime_failure_is_fail(self):
        # Even a fully self-consistent paper PASS cannot substitute for this runner's execution.
        evidence = json.loads((self.candidate / "evidence.json").read_bytes())
        for gate in oci.GATES:
            path = self.candidate / "gate-records" / (gate + ".json")
            record = {"gate": gate, "status": "PASS", "evidence_level": "integration-tested",
                      "execution_environment": "linux-native-exact-images",
                      **{k: self.manifest[k] for k in ("source_sha", "configuration_sha256", "platform", "images")}}
            path.write_bytes(contract.canonical(record) + b"\n")
            evidence["gates"][gate] = {"status": "PASS", "record_sha256": qualify.sha(path.read_bytes())}
        (self.candidate / "evidence.json").write_bytes(contract.canonical(evidence) + b"\n")
        manifest = copy.deepcopy(self.manifest)
        manifest["evidence"] = contract.file_record(self.candidate / "evidence.json", self.candidate)
        self.args.manifest.write_bytes(contract.canonical(manifest) + b"\n")
        self.args.manifest_digest = qualify.sha(self.args.manifest.read_bytes())
        # Existing verifier validates binding, not authenticity: operator review is still needed.
        oci.verify_evidence(self.candidate / "evidence.json", manifest)
        with patch.object(qualify, "native_host", return_value="linux/amd64"), \
                patch.object(qualify.shutil, "which", return_value="/fixture/tool"), \
                patch.object(qualify, "Fixture") as runtime, \
                patch.object(qualify, "run_gates", side_effect=ValueError("REAL_RUNTIME_FAILED")):
            result = qualify.qualify(self.root, self.args)
            runtime.return_value.close.assert_called_once()
        self.assertEqual(result["gates"]["image-pair"], "FAIL")
        self.assertEqual([result["gates"][g] for g in oci.GATES if g != "image-pair"], ["NOT_RUN"] * 3)
        with self.assertRaisesRegex(ValueError, "did not PASS"):
            oci.verify_evidence(self.args.output / "evidence.json", json.loads((self.args.output / "manifest.json").read_bytes()))

    def test_pulled_config_and_manifest_digest_and_fixed_uid_are_independent_guards(self):
        service = "cantos-server"
        image = self.manifest["images"][service]
        inspection = {"Os": "linux", "Architecture": "amd64", "Id": image["config_digest"],
                      "RepoDigests": [image["ref"]], "Config": {"User": "10001:10001",
                      "Entrypoint": ["/usr/local/bin/" + service], "Labels": {
                          "org.opencontainers.image.source": contract.REPOSITORY,
                          "org.opencontainers.image.revision": self.source,
                          "io.cantos.release.configuration": self.configuration, "io.cantos.binary": service}}}
        qualify.inspect_identity(service, image, self.manifest, inspection)
        for key, wrong, code in (("Id", "sha256:" + "a" * 64, "CONFIG_DIGEST"),
                                 ("RepoDigests", [], "REGISTRY_DIGEST")):
            altered = copy.deepcopy(inspection)
            altered[key] = wrong
            with self.assertRaisesRegex(ValueError, code):
                qualify.inspect_identity(service, image, self.manifest, altered)
        inspection["Config"]["User"] = "10002:10002"
        with self.assertRaisesRegex(ValueError, "UID"):
            qualify.inspect_identity(service, image, self.manifest, inspection)

    def test_all_layer_policy_detects_deleted_secrets_but_allows_ca_certificates_and_parser_strings(self):
        allowed = layer({"etc/ssl/certs/root.pem": b"-----BEGIN CERTIFICATE-----\nAAAA\n",
                         "usr/local/bin/cantos-server": b"parser: -----BEGIN PRIVATE KEY-----"})
        self.assertEqual(qualify.scan_layer(io.BytesIO(allowed)), {"cantos-server"})
        for name, data, code in (("root/.aws/credentials", b"synthetic", "FORBIDDEN_ASSET"),
                                 ("var/lib/server.db", b"synthetic", "FORBIDDEN_ASSET"),
                                 ("tmp/deleted-later.txt", b"-----BEGIN PRIVATE KEY-----\n" + b"A" * 64, "CREDENTIAL_MARKER"),
                                 ("../escape", b"x", "UNSAFE_PATH")):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, code):
                qualify.scan_layer(io.BytesIO(layer({name: data})))

    def test_cold_copy_preserves_all_sidecars_and_rejects_tampered_backup_or_links(self):
        source = self.work / "databases"
        source.mkdir(mode=0o700)
        for name, data in (("server.db", b"synthetic-turso-bytes"), ("server.db-wal", b"synthetic-wal"), ("server.db-shm", b"")):
            (source / name).write_bytes(data)
        backup = self.work / "backup"
        record = qualify.cold_copy(source, backup)
        self.assertEqual(record, qualify.directory_record(backup))
        (backup / "server.db-wal").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "PINNED_BACKUP_CHANGED"):
            qualify.cold_copy(backup, self.work / "restore", record)
        self.assertFalse((self.work / "restore").exists())
        (source / "link").symlink_to(source / "server.db")
        with self.assertRaisesRegex(ValueError, "REGULAR_FILES"):
            qualify.directory_record(source)

    def test_saved_oci_archive_binds_config_compressed_layer_and_uncompressed_diff_id(self):
        raw = layer({"usr/local/bin/cantos-server": b"synthetic executable",
                     "usr/local/bin/cantos-migrate": b"synthetic migration helper"})
        compressed = gzip.compress(raw)

        def archive(*, corrupt=False, diff_id=None):
            blobs = {}

            def descriptor(data, media):
                digest = "sha256:" + qualify.sha(data)
                blobs["blobs/sha256/" + digest[7:]] = data
                return {"digest": digest, "size": len(data), "mediaType": media}

            config = descriptor(contract.canonical({"rootfs": {"diff_ids": [diff_id or "sha256:" + qualify.sha(raw)]}}),
                                "application/vnd.oci.image.config.v1+json")
            item = descriptor(compressed, "application/vnd.oci.image.layer.v1.tar+gzip")
            manifest = descriptor(contract.canonical({"config": config, "layers": [item]}),
                                  "application/vnd.oci.image.manifest.v1+json")
            blobs["index.json"] = contract.canonical({"manifests": [manifest]})
            if corrupt:
                blobs["blobs/sha256/" + item["digest"][7:]] = b"X" + compressed[1:]
            path = self.work / "image.tar"
            path.write_bytes(layer(blobs))
            return path, {"config_digest": config["digest"]}

        path, image = archive()
        self.assertEqual(qualify.check_archive(path, "cantos-server", image)["layers"], 1)
        for change, code in (({"corrupt": True}, "BLOB_DIGEST"), ({"diff_id": "sha256:" + "f" * 64}, "DIFF_ID")):
            path, image = archive(**change)
            with self.assertRaisesRegex(ValueError, code):
                qualify.check_archive(path, "cantos-server", image)
        path, image = archive()
        with self.assertRaisesRegex(ValueError, "CONFIG_DIGEST"):
            qualify.check_archive(path, "cantos-server", {"config_digest": "sha256:" + "e" * 64})

    def test_serve_refusal_allows_only_the_private_empty_owner_lock_and_no_database(self):
        for service, uid in qualify.UIDS.items():
            owner = service.removeprefix("cantos-") + ".db.owner"
            record = {"bytes": 0, "sha256": qualify.sha(b""), "mode": 0o600, "uid": uid, "gid": uid}
            qualify.check_unmigrated_layout(service, {})
            qualify.check_unmigrated_layout(service, {owner: record})
            for field, wrong in (("bytes", 1), ("mode", 0o644), ("uid", 0), ("gid", 0)):
                with self.subTest(service=service, field=field), self.assertRaisesRegex(ValueError, "OWNER_LOCK_LAYOUT"):
                    qualify.check_unmigrated_layout(service, {owner: {**record, field: wrong}})
            for name in (owner.removesuffix(".owner"), "server.db-wal", "unknown"):
                with self.subTest(service=service, file=name), self.assertRaisesRegex(ValueError, "CREATED_DATABASE"):
                    qualify.check_unmigrated_layout(service, {owner: record, name: record})

    def test_pair_removal_orders_network_dependents_and_handles_an_already_stopped_identity(self):
        fixture = qualify.Fixture.__new__(qualify.Fixture)
        fixture.server, fixture.identity = "owned-server", "owned-identity"
        fixture.names = [fixture.identity, fixture.server]
        running = {fixture.server: True, fixture.identity: False}
        removed = []

        def pod(action, *args):
            name = args[-1]
            if action == "inspect":
                return json.dumps([{"State": {"Running": running[name]}}]).encode()
            if action == "stop":
                self.assertTrue(running[name], "an already stopped owner needs no stop command")
                running[name] = False
            elif action == "rm":
                self.assertFalse(running[name])
                if name == fixture.identity:
                    self.assertNotIn(fixture.server, running, "remove the network dependent first")
                removed.append(name)
                del running[name]
            return b""

        fixture.pod = pod  # Named command double; no Linux acceptance from this test.
        fixture.stop()
        self.assertEqual(removed, [fixture.server, fixture.identity])
        self.assertEqual(fixture.names, [])

    def test_podman_failure_identifies_operation_without_echoing_arguments_or_remote_output(self):
        fixture = qualify.Fixture.__new__(qualify.Fixture)
        fixture.podman = ["podman", "--root", "/private/owned-store"]
        with patch.object(fixture, "sh", side_effect=ValueError("COMMAND_podman_EXIT_125")):
            with self.assertRaisesRegex(ValueError, "^PODMAN_RM_EXIT_125$"):
                fixture.pod("rm", "owned-identity")
            fixture.operation_context = "stop-identity"
            with self.assertRaisesRegex(ValueError, "^stop-identity:PODMAN_RM_EXIT_125$"):
                fixture.pod("rm", "owned-identity")

    def test_wrong_peer_requires_actual_environment_and_static_ready_and_refusal_events(self):
        values = {**qualify.IDENTITY_ENV, "CANTOS_IDENTITY_SERVER_UIDS": "10003"}
        entries = [key + "=" + value for key, value in values.items()]
        qualify.check_runtime_environment("cantos-identity", entries, 10003)
        with self.assertRaisesRegex(ValueError, "ENVIRONMENT_MISMATCH"):
            qualify.check_runtime_environment("cantos-identity", entries, 10001)
        with self.assertRaisesRegex(ValueError, "ENVIRONMENT_AMBIGUOUS"):
            qualify.check_runtime_environment("cantos-identity", entries + [entries[0]], 10003)
        ready = b'{"event":"ready","mail":"configured"}\n'
        refused = b'{"event":"connection_refused","detail":"peer is not an allowed caller"}\n'
        self.assertEqual(qualify.peer_refusal_events(ready + refused),
                         ["identity-ready-event", "identity-connection-refused-event"])
        for data in (b"", ready, refused, ready + b'{"event":"unavailable"}\n'):
            with self.assertRaisesRegex(ValueError, "PEER_REFUSAL_NOT_OBSERVED"):
                qualify.peer_refusal_events(data)


if __name__ == "__main__":
    unittest.main()
