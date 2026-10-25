"""Producer contract checks on real Git/tar data; no Podman, registry, VPS or native E2E claim.

The build/push shell is exercised through a named command double (`mock-integration-tested`).
"""

import hashlib
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
import release_oci_build as build

PLATFORM = "linux/amd64"
ACTIONS_ENV = {"GITHUB_ACTIONS": "true", "GITHUB_REPOSITORY": "loveoverflowcom/cantos",
               "GITHUB_WORKFLOW_REF": "loveoverflowcom/cantos/.github/workflows/native-oci-publish.yml@refs/heads/main",
               "GITHUB_WORKFLOW_SHA": "e" * 40, "GITHUB_RUN_ID": "77", "GITHUB_RUN_ATTEMPT": "1",
               "GITHUB_JOB": "publish", "GITHUB_EVENT_NAME": "workflow_dispatch"}


def native_amd64_host():
    """Stand in for a native linux/amd64 builder whatever OS runs the test."""
    return patch.multiple(build.host_platform, system=lambda: "Linux", machine=lambda: "x86_64")


def git(root, *args):
    return subprocess.run(["git", "-c", "core.hooksPath=/dev/null", *args], cwd=root, check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.decode().strip()


def repository(root):
    files = {"services/cantos-server/src/platform/turso/mod.rs": b"pub const SERVER_EPOCH: u32 = 1;\n    version: 1,\n",
             "services/cantos-identity/src/persistence/mod.rs": b"    version: 1,\n",
             "rust-toolchain.toml": b"[toolchain]\nchannel='1.98.1'\n",
             "Cargo.lock": b'[[package]]\nname = "turso"\nversion = "0.8.3"\n',
             build.COMPATIBILITY: json.dumps({"rpc": "rpc-v1", "server_schema": "server-turso-epoch-1-v1",
                                              "identity_schema": "identity-migrations-v1",
                                              "security_state": "security-v1"}).encode(),
             "scripts/release_oci_build.py": b"#", "scripts/release_oci.py": b"#",
             "scripts/release_oci_registry.py": b"#", ".github/workflows/native-oci-publish.yml": b"#",
             "infra/oci/cantos-server/Containerfile": b"FROM scratch\n",
             "infra/oci/cantos-identity/Containerfile": b"FROM scratch\n",
             "infra/vps/bin/cantos-vps": b"#!/bin/sh\n", "infra/vps/quadlet/cantos-edge.container": b"[Container]\n",
             "infra/vps/postgres/roles.sql": b"-- not a host asset\n", "README.md": b"docs\n"}
    for name, data in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    (root / "infra/vps/bin/cantos-vps").chmod(0o755)
    git(root, "init", "--quiet")
    git(root, "add", ".")
    git(root, "-c", "user.name=Fixture", "-c", "user.email=f@example.invalid", "commit", "--quiet", "-m", "fixture")
    return git(root, "rev-parse", "HEAD")


def images():
    return {s: {"ref": REF.format(s, str(i + 1) * 64), "config_digest": "sha256:" + str(i + 5) * 64}
            for i, s in enumerate(oci.SERVICES)}


REF = "ghcr.io/loveoverflowcom/{}@sha256:{}"


class ProducerContract(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name) / "repo"
        self.root.mkdir()
        self.sha = repository(self.root)
        self.addCleanup(self.temporary.cleanup)

    def candidate(self):
        producer = build.producer_identity("local", {}, self.sha, "operator-1")
        config = build.configuration_identity(self.root, self.sha, PLATFORM)
        compat = build.compatibility(self.root, self.sha)
        members = build.host_members(self.root, self.sha)
        return producer, config, build.candidate(self.sha, config, PLATFORM, producer, compat, images(), members)

    def test_configuration_identity_changes_with_policy_platform_and_base_pins(self):
        base = build.configuration_identity(self.root, self.sha, PLATFORM)
        self.assertNotEqual(base, build.configuration_identity(self.root, self.sha, "linux/arm64"))
        (self.root / "infra/oci/cantos-server/Containerfile").write_bytes(b"FROM scratch\n# new base pin\n")
        git(self.root, "-c", "user.name=Fixture", "-c", "user.email=f@example.invalid", "commit", "-aqm", "pin")
        self.assertNotEqual(base, build.configuration_identity(self.root, git(self.root, "rev-parse", "HEAD"), PLATFORM))

    def test_schema_declaration_drift_is_refused_before_build(self):
        path = self.root / build.COMPATIBILITY
        declared = json.loads(path.read_text())
        declared['server_schema'] = 'server-turso-epoch-1-v23'
        path.write_text(json.dumps(declared))
        git(self.root, '-c', 'user.name=Fixture', '-c', 'user.email=f@example.invalid', 'commit', '-aqm', 'drift')
        with self.assertRaisesRegex(ValueError, 'differs from source registry'):
            build.compatibility(self.root, git(self.root, 'rev-parse', 'HEAD'))

    def test_host_bundle_has_only_committed_host_assets_with_git_modes(self):
        names = {name: mode for name, _, mode in build.host_members(self.root, self.sha)}
        self.assertEqual(names, {"REVISION": 0o644, "infra/vps/bin/cantos-vps": 0o755,
                                 "infra/vps/quadlet/cantos-edge.container": 0o644})

    def test_candidate_passes_the_release_contract_and_bundle_verification_but_never_evidence(self):
        producer, config, (manifest, bundle, evidence, records) = self.candidate()
        digest = hashlib.sha256(contract.canonical(manifest) + b"\n").hexdigest()
        admission = {"schema_version": 1, "manifest_sha256": digest, "producer": producer}
        oci.manifest_plan(manifest, admission, manifest_digest=digest, source_sha=self.sha,
                          configuration_digest=config, platform=PLATFORM)
        output = Path(self.temporary.name) / "out"
        written = build.write_candidate(output, manifest, bundle, evidence, records)
        self.assertEqual(written, digest)
        oci.verify_bundle(self.root, output / "runtime.tar", manifest)
        with self.assertRaisesRegex(ValueError, "did not PASS"):
            oci.verify_evidence(output / "evidence.json", manifest)
        self.assertEqual({r["status"] for r in map(json.loads, records.values())}, {"NOT_RUN"})

    def test_bundle_is_deterministic_and_modes_survive(self):
        members = build.host_members(self.root, self.sha)
        self.assertEqual(build.runtime_tar(members), build.runtime_tar(members))
        output = Path(self.temporary.name) / "bundle.tar"
        output.write_bytes(build.runtime_tar(members))
        with tarfile.open(output) as archive:
            self.assertEqual({e.name: e.mode for e in archive}["infra/vps/bin/cantos-vps"], 0o755)

    def test_actions_identity_must_match_repository_source_and_workflow(self):
        env = {**ACTIONS_ENV, "GITHUB_SHA": self.sha, "GITHUB_WORKFLOW_SHA": self.sha}
        self.assertEqual(build.producer_identity("github-actions", env, self.sha)["workflow_path"],
                         ".github/workflows/native-oci-publish.yml")
        for change in ({"GITHUB_SHA": "d" * 40}, {"GITHUB_WORKFLOW_SHA": "d" * 40},
                       {"GITHUB_RUN_ATTEMPT": "2"}, {"GITHUB_EVENT_NAME": "push"}, {"GITHUB_REPOSITORY": "other/cantos"},
                       {"GITHUB_WORKFLOW_REF": "loveoverflowcom/cantos/other.yml@refs/heads/main"},
                       {"GITHUB_RUN_ID": "0"}, {"GITHUB_ACTIONS": "false"}):
            with self.assertRaises(ValueError):
                build.producer_identity("github-actions", {**env, **change}, self.sha)
        with self.assertRaises(ValueError):
            build.producer_identity("github-actions", env, self.sha, "build-id-is-local-only")

    def test_local_image_must_satisfy_the_readback_contract_before_any_push(self):
        config = "c" * 64
        good = {"Os": "linux", "Architecture": "amd64", "Id": "a" * 64, "Config": {
            "User": "10001:10001", "Entrypoint": ["/usr/local/bin/cantos-server"], "Cmd": None,
            "Labels": {"org.opencontainers.image.source": contract.REPOSITORY,
                       "org.opencontainers.image.revision": self.sha,
                       "io.cantos.release.configuration": config, "io.cantos.binary": "cantos-server"}}}
        self.assertEqual(build.check_local_image("cantos-server", good, self.sha, config, PLATFORM), "sha256:" + "a" * 64)
        def broken(path, value):
            image = json.loads(json.dumps(good))
            target = image
            for key in path[:-1]:
                target = target[key]
            target[path[-1]] = value
            return image
        for image in (broken(("Config", "User"), "root"), broken(("Config", "User"), "0:0"),
                      broken(("Config", "Cmd"), ["sh"]), broken(("Config", "Entrypoint"), ["/bin/sh"]),
                      broken(("Architecture",), "arm64"), broken(("Config", "Labels", "io.cantos.binary"), "cantos-identity"),
                      broken(("Config", "Env"), ["API_TOKEN=x"]), broken(("Variant",), "v8")):
            with self.assertRaises(ValueError):
                build.check_local_image("cantos-server", image, self.sha, config, PLATFORM)

    def test_host_must_natively_match_the_platform_and_tree_must_be_clean(self):
        foreign = SimpleNamespace(system=lambda: "Linux", machine=lambda: "aarch64")
        with self.assertRaisesRegex(ValueError, "natively match"):
            build.preflight(self.root, PLATFORM, foreign)
        with self.assertRaisesRegex(ValueError, "platform must be"):
            build.preflight(self.root, "linux/riscv64", foreign)
        native = SimpleNamespace(system=lambda: "Linux", machine=lambda: "x86_64")
        self.assertEqual(build.preflight(self.root, PLATFORM, native), self.sha)
        (self.root / "stray.txt").write_text("x")
        with self.assertRaisesRegex(ValueError, "clean"):
            build.preflight(self.root, PLATFORM, native)


class Shell(unittest.TestCase):
    """The build/push shell against a command double: ordering and refusal, not real Podman."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name) / "repo"
        self.root.mkdir()
        self.sha = repository(self.root)
        self.addCleanup(self.temporary.cleanup)
        self.calls = []
        self.config = build.configuration_identity(self.root, self.sha, PLATFORM)
        self.authfile = Path(self.temporary.name) / "auth.json"
        self.authfile.write_text('{"auths":{"ghcr.io":{"auth":"dXNlcjpmaXh0dXJl"}}}')
        self.authfile.chmod(0o600)

    def fake(self, push_digest=None):
        real = build.sh

        def run(root, argv, *, env=None):
            if argv[0] == "podman" and argv[1] == "build":
                self.calls.append(("build", argv[argv.index("-f") + 1].split("/")[-2]))
                return ""
            if argv[0] == "podman" and argv[1:3] == ["image", "inspect"]:
                service = argv[3].split("/")[-1].split(":")[0]
                return json.dumps([{"Os": "linux", "Architecture": "amd64", "Id": "b" * 64, "Config": {
                    "User": "10001:10001", "Entrypoint": ["/usr/local/bin/" + service], "Cmd": None,
                    "Labels": {"org.opencontainers.image.source": contract.REPOSITORY,
                               "org.opencontainers.image.revision": self.sha,
                               "io.cantos.release.configuration": self.config, "io.cantos.binary": service}}}])
            if argv[0] == "podman" and argv[1] == "push":
                self.assertEqual(argv[-2], "sha256:" + "b" * 64, "push the inspected immutable image ID, not its local tag")
                self.calls.append(("push", argv[-1]))
                if push_digest is None:
                    raise ValueError("command failed: podman push")
                Path(argv[argv.index("--digestfile") + 1]).write_text(push_digest + "\n")
                return ""
            return real(root, argv, env=env)
        return run

    def args(self, publish, authfile=None):
        return SimpleNamespace(platform=PLATFORM, producer="local", build_id="operator-1", publish=publish,
                               authfile=authfile, output=Path("out"))

    def test_build_only_publishes_nothing_and_emits_no_manifest(self):
        with patch.object(build, "sh", self.fake()), native_amd64_host():
            result = build.produce(self.root, self.args(False))
        self.assertEqual(result["operation"], "BUILD_ONLY")
        self.assertEqual([c[0] for c in self.calls], ["build", "build"])
        self.assertEqual(result["gates"]["publish"], "NOT_RUN")
        self.assertFalse((self.root / "out").exists())

    def test_a_tree_dirtied_during_the_build_never_reaches_the_registry(self):
        fake = self.fake("sha256:" + "9" * 64)

        def dirtying(root, argv, *, env=None):
            result = fake(root, argv, env=env)
            if argv[0] == "podman" and argv[1] == "build":
                (self.root / "late.txt").write_text("x")
            return result
        with patch.object(build, "sh", dirtying), native_amd64_host():
            with self.assertRaisesRegex(ValueError, "clean"):
                build.produce(self.root, self.args(True, self.authfile))
        self.assertNotIn("push", [c[0] for c in self.calls])

    def test_failed_push_stops_without_a_manifest_and_never_retries(self):
        with patch.object(build, "sh", self.fake(None)), native_amd64_host():
            with self.assertRaises(ValueError):
                build.produce(self.root, self.args(True, self.authfile))
        self.assertEqual([c[0] for c in self.calls].count("push"), 1)
        journals = list((self.root / "out").rglob("publish-attempt.json"))
        self.assertEqual(len(journals), 1)
        self.assertEqual(json.loads(journals[0].read_text())["images"]["cantos-server"]["state"], "unknown")
        self.assertFalse(list((self.root / "out").rglob("manifest.json")))

    def test_publish_pushes_the_pair_by_tag_then_records_digest_refs_after_readback(self):
        digest = "sha256:" + "9" * 64
        observe = lambda manifest, plan, authfile=None: {**plan, "images": {}, "release_eligible": False,
                                                          "gates": {**plan["gates"], "registry-access": "PASS"}}
        with patch.object(build, "sh", self.fake(digest)), native_amd64_host(), \
                patch("release_oci_registry.observe", observe):
            result = build.produce(self.root, self.args(True, self.authfile))
        self.assertEqual(result["operation"], "CANDIDATE")
        self.assertFalse(result["release_eligible"])
        self.assertEqual(result["gates"]["native-qualification"], "NOT_RUN")
        manifest = json.loads((Path(result["output"]) / "manifest.json").read_text())
        self.assertEqual({s: manifest["images"][s]["ref"] for s in oci.SERVICES},
                         {s: build.REGISTRY + s + "@" + digest for s in oci.SERVICES})
        self.assertEqual([c for c in self.calls if c[0] == "push"],
                         [("push", "docker://" + build.REGISTRY + s + ":sha-" + self.sha) for s in oci.SERVICES])
        self.assertEqual(hashlib.sha256((Path(result["output"]) / "manifest.json").read_bytes()).hexdigest(),
                         result["manifest_sha256"])


if __name__ == "__main__":
    unittest.main()
