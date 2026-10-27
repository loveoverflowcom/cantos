"""Offline packaging regressions. Synthetic ELF is not Linux runtime evidence."""
import json
from pathlib import Path
import shutil
import stat
import struct
import tempfile
import unittest
from unittest.mock import patch

import forge_probe as probe


def elf(segment=1, machine=62):
    data = bytearray(152)
    data[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<HHI", data, 16, 2, machine, 1)
    struct.pack_into("<Q", data, 32, 64)
    struct.pack_into("<HH", data, 54, 56, 1)
    struct.pack_into("<IIQQQQQQ", data, 64, segment, 5, 120, 0, 0, 32, 32, 8)
    return bytes(data)


class ProbePackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="forge probe ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / probe.POLICY).parent.mkdir(parents=True)
        shutil.copyfile(probe.ROOT / probe.POLICY, self.root / probe.POLICY)
        source = self.root / probe.SOURCE
        source.mkdir(parents=True)
        for name in probe.RUNTIME_FILES:
            path = probe.ROOT / probe.SOURCE / name
            shutil.copyfile(path, source / name)
        (source / "test").mkdir()
        shutil.copyfile(probe.ROOT / probe.SOURCE / "test/packaged.test.mjs", source / "test/packaged.test.mjs")
        self.worker = self.root / "worker"
        self.worker.write_bytes(elf())
        self.worker.chmod(0o755)
        self.value = probe.policy(self.root)

    def stage(self):
        output = self.root / "artifacts/forge-probe/run with spaces"
        probe.stage(self.root, output, self.value, "asia-southeast1", self.worker,
                    "a" * 40, "b" * 64, "synthetic compiler fixture")
        return output

    def assertRefused(self, code, call):
        with self.assertRaisesRegex(probe.Refused, "^" + code + "$"):
            call()

    def test_config_and_manifest_allow_only_the_private_probe_source(self):
        output = self.stage()
        self.assertEqual(probe.read_json(output / "firebase.json"), {"functions": [{
            "source": "functions", "codebase": "cantos-forge-smoke", "runtime": "nodejs22",
            "ignore": ["node_modules", ".git", "firebase-debug.log", "firebase-debug.*.log"]}]})
        receipt = probe.inspect(output, self.root)
        self.assertEqual(receipt["worker"]["sha256"], probe.digest(elf()))
        self.assertEqual(receipt["cloud"], "NOT_RUN")
        self.assertEqual(stat.S_IMODE((output / "functions/bin/cantos-forge-worker").stat().st_mode), 0o755)
        self.assertRefused("output_exists", lambda: probe.destination("cantos-a348b", "asia-southeast1", output, self.root))

    def test_wrong_project_region_and_escape_are_refused(self):
        for project, region, output, error in [
            ("other-project", "asia-southeast1", "artifacts/forge-probe/run", "wrong_project"),
            ("cantos-a348b", "", "artifacts/forge-probe/run", "invalid_region"),
            ("cantos-a348b", "asia-southeast1", "../escape", "unsafe_output"),
            ("cantos-a348b", "asia-southeast1", "artifacts/forge-probe", "unsafe_output"),
        ]:
            self.assertRefused(error, lambda: probe.destination(project, region, output, self.root))

    def test_linked_staging_parent_cannot_escape_the_workspace(self):
        outside = self.root / "outside"
        outside.mkdir()
        (self.root / "artifacts").mkdir()
        (self.root / "artifacts/forge-probe").symlink_to(self.root.parent, target_is_directory=True)
        self.assertRefused("unsafe_output", lambda: probe.destination("cantos-a348b", "asia-southeast1",
                           "artifacts/forge-probe/new", self.root))

    def test_interrupted_staging_is_preserved_and_cannot_be_reused(self):
        output = self.root / "artifacts/forge-probe/interrupted"
        output.mkdir(parents=True)
        (output / "partial").write_text("preserve")
        with self.assertRaises(FileExistsError):
            probe.stage(self.root, output, self.value, "asia-southeast1", self.worker,
                        "a" * 40, "b" * 64, "fixture")
        self.assertEqual((output / "partial").read_text(), "preserve")

    def test_binary_rejects_mac_arm_dynamic_unexecutable_and_truncated_inputs(self):
        self.assertRefused("worker_not_linux_elf64", lambda: probe.static_linux_elf(b"macOS" * 20, 0o755))
        self.assertRefused("worker_wrong_architecture", lambda: probe.static_linux_elf(elf(machine=183), 0o755))
        self.assertRefused("worker_dynamic_loader", lambda: probe.static_linux_elf(elf(segment=3), 0o755))
        for mode in (0o644, 0o777, 0o775):
            self.assertRefused("worker_permissions", lambda: probe.static_linux_elf(elf(), mode))
        for size in range(len(elf())):
            with self.assertRaises(probe.Refused):
                probe.static_linux_elf(elf()[:size], 0o755)
        data = bytearray(elf(segment=2))
        struct.pack_into("<qQ", data, 120, 1, 0)
        self.assertRefused("worker_dynamic_dependency", lambda: probe.static_linux_elf(data, 0o755))

    def test_tampered_missing_and_unexpected_files_never_inspect_successfully(self):
        output = self.stage()
        worker = output / "functions/bin/cantos-forge-worker"
        worker.write_bytes(elf(machine=183))
        self.assertRefused("bundle_hash_mismatch", lambda: probe.inspect(output, self.root))
        worker.write_bytes(elf())
        (output / "functions/secret.json").write_text("synthetic")
        self.assertRefused("unexpected_bundle_file", lambda: probe.inspect(output, self.root))
        (output / "functions/secret.json").unlink()
        (output / "functions/http.mjs").unlink()
        self.assertRefused("unexpected_bundle_file", lambda: probe.inspect(output, self.root))

    def test_symlink_and_mode_substitution_are_refused(self):
        output = self.stage()
        worker = output / "functions/bin/cantos-forge-worker"
        worker.chmod(0o644)
        self.assertRefused("worker_permissions", lambda: probe.inspect(output, self.root))
        worker.unlink()
        worker.symlink_to(self.worker)
        self.assertRefused("bundle_symlink", lambda: probe.inspect(output, self.root))

    def test_dry_run_has_no_subprocess_or_output_effect(self):
        args = type("Args", (), {"project": "cantos-a348b", "region": "asia-southeast1",
                                 "output": "artifacts/forge-probe/dry-run", "dry_run": True})()
        with patch.object(probe, "ROOT", self.root), patch.object(probe, "destination", return_value=(self.value, self.root / args.output)), patch.object(probe, "invoke", side_effect=AssertionError("subprocess")):
            self.assertEqual(probe.package(args), 0)
        self.assertFalse((self.root / args.output).exists())

    def test_effective_config_cannot_be_rewritten_to_hosting_even_with_updated_hash(self):
        output = self.stage()
        probe.write_json(output / "firebase.json", {"hosting": {"public": "functions"}})
        receipt = probe.read_json(output / "receipt.json")
        receipt["files"]["firebase.json"] = probe.digest((output / "firebase.json").read_bytes())
        probe.write_json(output / "receipt.json", receipt)
        self.assertRefused("invalid_effective_config", lambda: probe.inspect(output, self.root))


if __name__ == "__main__":
    unittest.main()
