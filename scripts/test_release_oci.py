"""Real local Git/tar contract checks; no OCI, registry, VPS or native E2E claim."""

import copy
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_oci as oci


CONFIG = "c" * 64
PRODUCERS = ({"kind": "local", "build_id": "operator-build-1"},
             {"kind": "github-actions", "workflow_sha": "f" * 40,
              "workflow_path": ".github/workflows/native-release.yml", "run_id": "42",
              "run_attempt": "1", "job_id": "publish"})


def git(root, *args):
    return subprocess.run(["git", "-c", "core.hooksPath=/dev/null", *args], cwd=root, check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.decode().strip()


def fixture(root, producer=PRODUCERS[0]):
    source = root / "source"
    source.mkdir()
    helper = source / "infra/vps/bin/cantos-vps"
    helper.parent.mkdir(parents=True)
    helper.write_bytes(b"#!/bin/sh\nexit 0\n")
    helper.chmod(0o755)
    server=source/'services/cantos-server/src/platform/turso/mod.rs'
    identity=source/'services/cantos-identity/src/persistence/mod.rs'
    server.parent.mkdir(parents=True);identity.parent.mkdir(parents=True)
    server.write_text('pub const SERVER_EPOCH: u32 = 1;\n    version: 1,\n')
    identity.write_text('    version: 1,\n')
    policy=source/'infra/oci/compatibility.json';policy.parent.mkdir(parents=True)
    policy.write_text(json.dumps({'rpc':'rpc-v1','server_schema':'server-turso-epoch-1-v1',
                                 'identity_schema':'identity-migrations-v1','security_state':'security-v1'}))
    (source/'Cargo.lock').write_text('[[package]]\nname = "turso"\nversion = "0.6.0-pre.7"\n')
    git(source, "init", "--quiet")
    git(source, "add", "infra/vps/bin/cantos-vps", "infra/oci/compatibility.json", "Cargo.lock", "services")
    git(source, "-c", "user.name=Contract Fixture", "-c", "user.email=fixture@example.invalid", "commit", "--quiet", "-m", "fixture")
    sha = git(source, "rev-parse", "HEAD")
    release = root / "release"
    release.mkdir()
    members = [("REVISION", (sha + "\n").encode(), 0o644),
               ("infra/vps/bin/cantos-vps", helper.read_bytes(), 0o755)]
    bundle = release / "runtime.tar"
    with tarfile.open(bundle, "w") as archive:
        for name, data, mode in members:
            entry = tarfile.TarInfo(name)
            entry.size, entry.mode = len(data), mode
            archive.addfile(entry, io.BytesIO(data))
    manifest = {"schema_version": 1, "kind": "cantos-native-oci", "repository": contract.REPOSITORY,
                "source_sha": sha, "configuration_sha256": CONFIG, "platform": "linux/amd64",
                "producer": copy.deepcopy(producer),
                "compatibility": {"rpc": "rpc-v1", "server_schema": "server-turso-epoch-1-v1", "identity_schema": "identity-migrations-v1",
                                  "security_state": "security-v1", "turso": "0.6.0-pre.7"},
                "images": {service: {"ref": "ghcr.io/loveoverflowcom/" + service + "@sha256:" + str(i + 1) * 64,
                                     "config_digest": "sha256:" + str(i + 3) * 64}
                           for i, service in enumerate(oci.SERVICES)},
                "runtime_bundle": {**contract.file_record(bundle, release),
                                   "entries": [{"path": name, "bytes": len(data), "mode": mode,
                                                "sha256": hashlib.sha256(data).hexdigest()}
                                               for name, data, mode in members]}}
    gate_records = release / "gate-records"
    gate_records.mkdir()
    for gate in oci.GATES:
        contract.write_new(gate_records / (gate + ".json"), {"gate": gate, "status": "PASS",
            **{key: manifest[key] for key in ("source_sha", "configuration_sha256", "platform", "images")}})
    evidence = {"schema_version": 1, **{key: manifest[key] for key in
                ("source_sha", "configuration_sha256", "platform", "images", "producer")},
                "gates": {gate: {"status": "PASS", "record_sha256": contract.file_record(gate_records / (gate + ".json"), gate_records)["sha256"]} for gate in oci.GATES}}
    contract.write_new(release / "evidence.json", evidence)
    manifest["evidence"] = contract.file_record(release / "evidence.json", release)
    path = release / "manifest.json"
    contract.write_new(path, manifest)
    args = SimpleNamespace(manifest=path, manifest_digest=contract.file_record(path, release)["sha256"],
                           source_sha=sha, configuration_digest=CONFIG, platform="linux/amd64",
                           admission=release / "admission.json", dry_run=False)
    admission = {"schema_version": 1, "manifest_sha256": args.manifest_digest, "producer": manifest["producer"]}
    contract.write_new(args.admission, admission)
    args.admission_digest = contract.file_record(args.admission, release)["sha256"]
    return source, args, manifest, admission


class OciContractTests(unittest.TestCase):
    def pure(self, manifest, admission, args, **changes):
        pins = {name: getattr(args, name) for name in
                ("manifest_digest", "source_sha", "configuration_digest", "platform")}
        return oci.manifest_plan(manifest, admission, **(pins | changes))

    def test_both_producers_have_the_same_existing_artifact_consumer_and_no_effects(self):
        for producer in PRODUCERS:
            with self.subTest(producer=producer["kind"]), tempfile.TemporaryDirectory() as temporary:
                source, args, manifest, _ = fixture(Path(temporary), producer)
                calls = []
                real = subprocess.run
                def git_only(argv, **kwargs):
                    calls.append(argv)
                    self.assertEqual(argv[0], "git")
                    self.assertEqual(argv[1:4], ["--no-lazy-fetch", "--no-optional-locks", "--no-replace-objects"])
                    self.assertIn(argv[4], ("cat-file", "ls-tree", "show"))
                    return real(argv, **kwargs)
                before = {p.relative_to(Path(temporary)): p.read_bytes() for p in args.manifest.parent.rglob("*") if p.is_file()}
                with patch.object(oci.subprocess, "run", side_effect=git_only):
                    normal = oci.verify(source, args)
                    args.dry_run = True
                    self.assertEqual(oci.verify(source, args), normal)
                self.assertTrue(calls)
                self.assertEqual(normal["operation"], "PLAN_ONLY")
                self.assertEqual(normal["images"], {s: manifest["images"][s]["ref"] for s in oci.SERVICES})
                self.assertEqual(normal["gates"], {"offline-contract": "PASS", "registry-access": "NOT_RUN",
                                                 "native-qualification": "NOT_RUN", "activation": "NOT_RUN", "cleanup": "NOT_RUN"})
                self.assertEqual(before, {p.relative_to(Path(temporary)): p.read_bytes() for p in args.manifest.parent.rglob("*") if p.is_file()})

    def test_admission_is_exact_and_does_not_come_from_a_hash_or_a_label(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, args, manifest, admission = fixture(Path(temporary))
            for mutation in (lambda a: a.update(manifest_sha256="0" * 64),
                             lambda a: a.update(producer=PRODUCERS[1]),
                             lambda a: a.update(schema_version=True)):
                changed = copy.deepcopy(admission)
                mutation(changed)
                with self.assertRaisesRegex(ValueError, "producer admission mismatch"):
                    self.pure(manifest, changed, args)

    def test_wrong_source_config_platform_and_missing_pair_are_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, args, manifest, admission = fixture(Path(temporary))
            for pins, reason in (({"source_sha": "a" * 40}, "source SHA mismatch"),
                                 ({"configuration_digest": "a" * 64}, "configuration identity mismatch"),
                                 ({"platform": "linux/arm64"}, "platform mismatch"),
                                 ({"platform": "darwin/arm64"}, "platform mismatch")):
                with self.subTest(pins=pins), self.assertRaisesRegex(ValueError, reason):
                    self.pure(manifest, admission, args, **pins)
            del manifest["images"]["cantos-identity"]
            with self.assertRaisesRegex(ValueError, "exact Server/Identity image pair"):
                self.pure(manifest, admission, args)

    def test_tags_wrong_registry_swapped_services_config_ids_and_indexes_are_refused(self):
        mutations = ((lambda i: i.update(ref="ghcr.io/loveoverflowcom/cantos-server:latest"), "GHCR digest"),
                     (lambda i: i.update(ref="other.invalid/cantos-server@sha256:" + "1" * 64), "GHCR digest"),
                     (lambda i: i.update(ref="ghcr.io/loveoverflowcom/cantos-identity@sha256:" + "1" * 64), "GHCR digest"),
                     (lambda i: i.update(config_digest="sha256:" + "1" * 64), "config/image ID"),
                     (lambda i: i.update(index_digest="sha256:" + "5" * 64), "indexes unsupported"))
        with tempfile.TemporaryDirectory() as temporary:
            _, args, manifest, admission = fixture(Path(temporary))
            for mutation, reason in mutations:
                changed = copy.deepcopy(manifest)
                mutation(changed["images"]["cantos-server"])
                with self.subTest(reason=reason), self.assertRaisesRegex(ValueError, reason):
                    self.pure(changed, admission, args)

    def test_unsupported_producer_and_incomplete_compatibility_fail_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, args, manifest, admission = fixture(Path(temporary))
            for kind in ("existing", "circleci", "", None):
                changed = copy.deepcopy(manifest)
                changed["producer"]["kind"] = kind
                with self.assertRaisesRegex(ValueError, "producer must be"):
                    self.pure(changed, admission, args)
            manifest["compatibility"].pop("security_state")
            with self.assertRaisesRegex(ValueError, "compatibility identity"):
                self.pure(manifest, admission, args)

    def test_bundle_records_refuse_unowned_paths_duplicates_modes_and_unbounded_sizes(self):
        mutations = ((lambda e: e.update(path="../../outside"), "unsafe archive path"),
                     (lambda e: e.update(path="/etc/hosts"), "unsafe archive path"),
                     (lambda e: e.update(path="private-assets/story.txt"), "unexpected host bundle asset"),
                     (lambda e: e.update(path="infra/vps/.env"), "unexpected host bundle asset"),
                     (lambda e: e.update(path="infra/vps/config/identity.db"), "unexpected host bundle asset"),
                     (lambda e: e.update(path="infra/vps/config.env"), "unexpected host bundle asset"),
                     (lambda e: e.update(path="REVISION"), "duplicate bundle member"),
                     (lambda e: e.update(mode=0o644), "must be executable"),
                     (lambda e: e.update(mode=0o4755), "bundle modes"),
                     (lambda e: e.update(bytes=True), "member size"),
                     (lambda e: e.update(bytes=oci.MAX_BUNDLE_BYTES + 1), "member size"))
        with tempfile.TemporaryDirectory() as temporary:
            _, args, manifest, admission = fixture(Path(temporary))
            for mutation, reason in mutations:
                changed = copy.deepcopy(manifest)
                mutation(changed["runtime_bundle"]["entries"][1])
                with self.subTest(reason=reason), self.assertRaisesRegex(ValueError, reason):
                    self.pure(changed, admission, args)

    def test_bundle_archive_refuses_links_duplicates_mode_revision_and_source_drift(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, args, manifest, _ = fixture(Path(temporary))
            bundle = args.manifest.parent / "runtime.tar"
            original = bundle.read_bytes()
            for scenario, reason in (("symlink", "must be regular"), ("duplicate", "duplicate archive entry"),
                                     ("mode", "size/mode mismatch"), ("revision", "source revision mismatch"),
                                     ("source", "committed source bytes"), ("hardlink", "must be regular")):
                with tarfile.open(fileobj=io.BytesIO(original)) as archive:
                    members = [(copy.copy(e), archive.extractfile(e).read()) for e in archive]
                if scenario in ("symlink", "hardlink"):
                    members[1][0].type = tarfile.SYMTYPE if scenario == "symlink" else tarfile.LNKTYPE
                    members[1][0].linkname = "/etc/hosts"
                if scenario == "duplicate":
                    members.append(members[1])
                if scenario == "mode":
                    members[1][0].mode = 0o644
                if scenario in ("revision", "source"):
                    index = 0 if scenario == "revision" else 1
                    entry, data = members[index]
                    members[index] = (entry, b"x" * len(data))
                with tarfile.open(bundle, "w") as archive:
                    for entry, data in members:
                        archive.addfile(entry, io.BytesIO(data))
                changed = copy.deepcopy(manifest)
                changed["runtime_bundle"].update(contract.file_record(bundle, bundle.parent))
                if scenario in ("revision", "source"):
                    changed["runtime_bundle"]["entries"][index]["sha256"] = hashlib.sha256(members[index][1]).hexdigest()
                with self.subTest(scenario=scenario), self.assertRaisesRegex(ValueError, reason):
                    oci.verify_bundle(source, bundle, changed)

    def test_corrupt_bytes_and_changed_evidence_cannot_be_admitted_by_metadata_alone(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, args, manifest, _ = fixture(Path(temporary))
            evidence_path = args.manifest.parent / "evidence.json"
            original = evidence_path.read_bytes()
            for gate in oci.GATES:
                for status in ("FAIL", "NOT_RUN", "UNSUPPORTED"):
                    evidence = contract.read_json(evidence_path)
                    evidence["gates"][gate]["status"] = status
                    evidence_path.write_bytes(contract.canonical(evidence))
                    manifest["evidence"] = contract.file_record(evidence_path, evidence_path.parent)
                    with self.assertRaisesRegex(ValueError, "producer gate did not PASS: " + gate):
                        oci.verify_evidence(evidence_path, manifest)
                    evidence_path.write_bytes(original)
            manifest["evidence"] = contract.file_record(evidence_path, evidence_path.parent)
            evidence = contract.read_json(evidence_path)
            evidence["images"]["cantos-identity"]["ref"] = "ghcr.io/loveoverflowcom/cantos-identity@sha256:" + "6" * 64
            evidence_path.write_bytes(contract.canonical(evidence))
            manifest["evidence"] = contract.file_record(evidence_path, evidence_path.parent)
            with self.assertRaisesRegex(ValueError, "evidence release identity mismatch"):
                oci.verify_evidence(evidence_path, manifest)
            (args.manifest.parent / "runtime.tar").write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "changed before staging"):
                oci.verify_bundle(source, args.manifest.parent / "runtime.tar", manifest)

    def test_manifest_and_admission_hashes_duplicate_json_and_symlink_paths_are_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, args, _, _ = fixture(Path(temporary))
            for name in ("manifest_digest", "admission_digest"):
                original = getattr(args, name)
                setattr(args, name, "0" * 64)
                with self.assertRaisesRegex(ValueError, "manifest digest mismatch"):
                    oci.verify(source, args)
                setattr(args, name, original)
            args.admission.write_bytes(b'{"schema_version":1,"schema_version":1}')
            args.admission_digest = contract.file_record(args.admission, args.admission.parent)["sha256"]
            with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
                oci.verify(source, args)
            original = args.manifest
            args.manifest = original.parent / "link.json"
            args.manifest.symlink_to(original)
            with self.assertRaisesRegex(ValueError, "symlink in release path"):
                oci.verify(source, args)

    def test_invalid_identity_stops_before_any_git_or_host_effect(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, args, _, _ = fixture(Path(temporary))
            args.source_sha = "a" * 40
            with patch.object(oci.subprocess, "run", side_effect=AssertionError("unexpected effect")):
                with self.assertRaisesRegex(ValueError, "source SHA mismatch"):
                    oci.verify(source, args)

    def test_oversized_or_grown_bundle_is_bounded_before_snapshot_writes(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, args, manifest, _ = fixture(Path(temporary))
            bundle = args.manifest.parent / "runtime.tar"
            with bundle.open("r+b") as stream:
                stream.truncate(oci.MAX_BUNDLE_BYTES + 1)
            with self.assertRaisesRegex(ValueError, "bounded nonempty regular file"):
                oci.verify_bundle(source, bundle, manifest)
            # A source may grow after metadata was read. The shared copier must stop
            # before writing beyond the admitted length, even inside its general cap.
            bundle.write_bytes(b"x" * (1024 * 1024 + 1))
            destination = Path(temporary) / "snapshot.tar"
            expected = {"path": "snapshot.tar", "bytes": 1, "sha256": "a" * 64}
            with self.assertRaisesRegex(ValueError, "changed or exceeds release limit"):
                contract.copy_verified(bundle, destination, expected, limit=oci.MAX_BUNDLE_BYTES)
            self.assertEqual(destination.stat().st_size, 0)

    def test_missing_promisor_blob_refuses_without_transport_or_object_store_changes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, args, _, _ = fixture(root)
            blob = git(source, "rev-parse", "HEAD:infra/vps/bin/cantos-vps")
            git(source, "remote", "add", "origin", "cantos-recording::fixture")
            git(source, "config", "remote.origin.promisor", "true")
            git(source, "config", "remote.origin.partialclonefilter", "blob:none")
            git(source, "config", "protocol.cantos-recording.allow", "always")
            (source / ".git/objects" / blob[:2] / blob[2:]).unlink()
            marker = root / "transport-attempts"
            executable = root / "bin/git-remote-cantos-recording"
            executable.parent.mkdir()
            executable.write_text('#!/bin/sh\nprintf invoked >>"' + str(marker) + '"\nexit 1\n')
            executable.chmod(0o755)
            trace = root / "git-events.jsonl"
            env = {"PATH": str(executable.parent) + os.pathsep + os.environ.get("PATH", ""),
                   "GIT_NO_LAZY_FETCH": "0", "GIT_ALLOW_PROTOCOL": "cantos-recording",
                   "GIT_TRACE2_EVENT": str(trace)}
            def snapshot():
                paths = [source / ".git", *(source / ".git").rglob("*")]
                return {p.relative_to(source): (p.stat().st_mode, p.stat().st_mtime_ns,
                                               p.read_bytes() if p.is_file() else None) for p in paths}
            with patch.dict(os.environ, env):
                # Independent sensitivity control: ordinary Git really tries this
                # finite, local recording transport when the promised blob is absent.
                control = subprocess.run(["git", "show", args.source_sha + ":infra/vps/bin/cantos-vps"],
                                         cwd=source, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
                self.assertNotEqual(control.returncode, 0)
                self.assertTrue(marker.read_text())
                self.assertEqual(marker.read_text().replace("invoked", ""), "")
                marker.unlink()
                trace.unlink()
                before = snapshot()
                for dry_run in (False, True):
                    args.dry_run = dry_run
                    with self.assertRaises(ValueError) as refusal:
                        oci.verify(source, args)
                self.assertFalse(marker.exists())
                self.assertIn("locally present source objects", str(refusal.exception))
                after = snapshot()
                self.assertEqual(before, after)
                events = [json.loads(line) for line in trace.read_text().splitlines()]
                self.assertFalse([e for e in events if e.get("event") == "child_start"], events)

    def test_a_tree_sha_is_not_a_source_commit_even_when_bundle_bytes_and_modes_match(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, args, manifest, _ = fixture(Path(temporary))
            tree = git(source, "rev-parse", "HEAD^{tree}")
            bundle = args.manifest.parent / "runtime.tar"
            with tarfile.open(bundle) as archive:
                members = [(copy.copy(e), archive.extractfile(e).read()) for e in archive]
            members[0] = (members[0][0], (tree + "\n").encode())
            with tarfile.open(bundle, "w") as archive:
                for entry, data in members:
                    archive.addfile(entry, io.BytesIO(data))
            manifest["source_sha"] = tree
            manifest["runtime_bundle"].update(contract.file_record(bundle, bundle.parent))
            manifest["runtime_bundle"]["entries"][0]["sha256"] = hashlib.sha256(members[0][1]).hexdigest()
            with self.assertRaisesRegex(ValueError, "source SHA must name a commit object"):
                oci.verify_bundle(source, bundle, manifest)

    def test_git_without_no_lazy_fetch_support_is_refused_without_unguarded_fallback(self):
        refusal = subprocess.CompletedProcess([], 129, b"", b"unknown option: --no-lazy-fetch")
        with patch.object(oci.subprocess, "run", return_value=refusal) as run:
            with self.assertRaisesRegex(ValueError, "Git --no-lazy-fetch support"):
                oci.git(Path.cwd(), "cat-file", "-t", "a" * 40)
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args.args[0][1], "--no-lazy-fetch")
        self.assertEqual(run.call_args.kwargs["env"]["GIT_NO_LAZY_FETCH"], "1")
        self.assertEqual(run.call_args.kwargs["env"]["GIT_ALLOW_PROTOCOL"], "")


if __name__ == "__main__":
    unittest.main()
