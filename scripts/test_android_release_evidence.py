"""Audit pairing against synthetic artifact bytes, never an actual APK size measurement."""
import json
from pathlib import Path
import tempfile
import unittest
import zipfile
from unittest.mock import patch

import android_release_evidence as evidence
import inspect_android_release as inspector


class AuditEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.mobile = Path(self.temporary.name)
        for mode in ("build", "build-audit"):
            base = self.mobile / "androidApp" / mode
            for relative, data in {
                "outputs/apk/release/app.apk": b"optimized" if mode == "build" else b"unshrunk synthetic bytes",
                "outputs/apk/release/output-metadata.json": json.dumps({"applicationId":"com.cantos.theatre",
                    "variantName":"release", "elements":[{"type":"SINGLE", "filters":[], "outputFile":"app.apk"}]}).encode(),
                "outputs/bundle/release/androidApp-release.aab": b"synthetic bundle",
                "outputs/mapping/release/mapping.txt": b"# pg_map_id: ab123cd\ncom.cantos.Real -> a.b:\n",
                "reports/release-boundary/resolved-dependencies.txt": b"same release graph",
                "reports/release-boundary/pre-shrink.json": json.dumps({"status":"PASS", "variant":"release",
                    "class_count":10,"classes_sha256":"a" * 64,"inputs_sha256":"e" * 64,"source_sha256":"b" * 64}).encode(),
                "reports/release-boundary/artifact-inspection.txt": b"synthetic inspector report",
            }.items():
                path = base / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)

            for relative, dex in (("outputs/apk/release/app.apk", "classes.dex"),
                                  ("outputs/bundle/release/androidApp-release.aab", "base/dex/classes.dex")):
                with zipfile.ZipFile(base / relative, "w") as archive:
                    archive.writestr(dex, b'{"pg-map-id":"ab123cd"}' + mode.encode())

            report = f"variant=release audit={mode == 'build-audit'}\n"
            for kind, path in (("apk", base / "outputs/apk/release/app.apk"),
                               ("aab", base / "outputs/bundle/release/androidApp-release.aab")):
                report += f"{kind} synthetic sha256={evidence.sha(path)} entries=1 defined_classes=1\n"
            report += "PASS: no sandbox, mock, demo or test content found\n"
            (base / "reports/release-boundary/artifact-inspection.txt").write_text(report)

    def bind(self):
        with patch.object(evidence.subprocess, "run") as run:
            run.return_value.stdout = "c" * 40
            return evidence.bind(self.mobile, self.mobile)

    def test_pair_binds_separate_bytes_mapping_inputs_and_metric(self):
        result = self.bind()
        self.assertFalse(result["artifacts"]["audit"]["distributable"])
        self.assertNotEqual(result["artifacts"]["audit"]["apk"]["sha256"], result["artifacts"]["optimized"]["apk"]["sha256"])
        self.assertEqual(result["size"]["metric"], "APK archive bytes")
        self.assertEqual(result["size"]["installed_footprint"], "NOT_RUN")
        self.assertEqual(result["device_acceptance"], "NOT_RUN")

    def test_graph_classes_or_source_disagreement_is_rejected(self):
        report = self.mobile / "androidApp/build-audit/reports/release-boundary/pre-shrink.json"
        original = report.read_text()
        for field in ("source_sha256", "classes_sha256", "inputs_sha256", "class_count"):
            record = json.loads(original)
            record[field] = 1 if field == "class_count" else "changed"
            report.write_text(json.dumps(record))
            with self.assertRaisesRegex(ValueError, "mismatch"):
                self.bind()
        report.write_text(original)
        report.with_name("resolved-dependencies.txt").write_bytes(b"new transitive graph")
        with self.assertRaisesRegex(ValueError, "graph mismatch"):
            self.bind()

    def test_wrong_r8_mapping_is_rejected_before_snapshot(self):
        path = self.mobile / "androidApp/build/outputs/mapping/release/mapping.txt"
        path.write_text("# pg_map_id: wrong\ncom.cantos.Real -> a.b:\n")
        with self.assertRaisesRegex(inspector.InspectionError, "mapping ID does not match"):
            self.bind()

    def test_snapshot_survives_gradle_output_replacement_and_rejects_tampering(self):
        import release_artifact as contract
        result = self.bind()
        snapshot = evidence.freeze(self.mobile, self.mobile, result)
        (self.mobile / "androidApp/build/outputs/apk/release/app.apk").write_bytes(b"later build")
        contract.validate_android_outputs(snapshot, "c" * 40)
        copied = snapshot / "androidApp-release-unsigned.apk"
        copied.chmod(0o600)
        copied.write_bytes(b"replaced snapshot")
        with self.assertRaisesRegex(ValueError, "differs from inspected"):
            contract.validate_android_outputs(snapshot, "c" * 40)

    def test_inspection_report_cannot_attest_changed_artifact_bytes(self):
        (self.mobile / "androidApp/build/outputs/apk/release/app.apk").write_bytes(b"replaced after inspection")
        with self.assertRaisesRegex(ValueError, "inspection report/artifact hash mismatch"):
            self.bind()

    def test_missing_mapping_or_ambiguous_output_fails(self):
        metadata = self.mobile / "androidApp/build/outputs/apk/release/output-metadata.json"
        value = json.loads(metadata.read_text())
        value["elements"] *= 2
        metadata.write_text(json.dumps(value))
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            self.bind()
        value["elements"] = value["elements"][:1]
        metadata.write_text(json.dumps(value))
        (self.mobile / "androidApp/build/outputs/mapping/release/mapping.txt").unlink()
        with self.assertRaisesRegex(ValueError, "missing"):
            self.bind()

    def test_ci_staging_cannot_change_mapping_or_apk_after_inspection(self):
        import shutil
        import release_artifact as contract
        result = self.bind()
        output = self.mobile / "packaged"
        output.mkdir()
        for mode, directory in (("audit", "build-audit"), ("optimized", "build")):
            base = self.mobile / "androidApp" / directory
            prefix = "audit-" if mode == "audit" else ""
            for relative, name in [("outputs/apk/release/app.apk", "androidApp-release-unsigned.apk"),
                ("outputs/bundle/release/androidApp-release.aab", "androidApp-release.aab"),
                ("reports/release-boundary/artifact-inspection.txt", "artifact-inspection.txt"),
                ("reports/release-boundary/resolved-dependencies.txt", "resolved-dependencies.txt"),
                ("reports/release-boundary/pre-shrink.json", "pre-shrink.json")]:
                shutil.copyfile(base / relative, output / (prefix + name))
        shutil.copyfile(self.mobile / "androidApp/build/outputs/mapping/release/mapping.txt", output / "mapping.txt")
        (output / "release-evidence.json").write_text(json.dumps(result))
        contract.validate_android_outputs(output, "c" * 40)
        for name in ("mapping.txt", "androidApp-release-unsigned.apk", "audit-androidApp-release-unsigned.apk"):
            original = (output / name).read_bytes()
            (output / name).write_bytes(b"changed bytes")
            with self.assertRaisesRegex(ValueError, "mapping mismatch|differs from inspected"):
                contract.validate_android_outputs(output, "c" * 40)
            (output / name).write_bytes(original)

    def test_audit_marker_cannot_be_laundered_by_renaming_apk(self):
        for marker in ("true", "0xffffffff", "-1", None):
            elements = [(0,"manifest",{}), (2,"data",{"scheme":"com.cantos.theatre"})]
            if marker is not None:
                elements.append((2,"meta-data",{"name":"CantosReleaseAudit", "value":marker}))
            self.assertEqual(inspector.release_metadata_findings(elements, "com.cantos.theatre", audit=False),
                ["missing or wrong audit marker; distribution refuses audit bytes"])
        elements = [(0,"manifest",{}),(2,"data",{"scheme":"com.cantos.theatre.dev"}),
                    (2,"meta-data",{"name":"CantosReleaseAudit","value":"false"})]
        self.assertEqual(inspector.release_metadata_findings(elements,"com.cantos.theatre", audit=False),
                         ["callback scheme differs from application identity"])
