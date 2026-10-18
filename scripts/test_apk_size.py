"""Synthetic APKs and analyzer text exercise accounting, deltas and fail-closed results."""

import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import apk_size
from apk_size import (ToolFailure, build_report, classify, compare_reports, file_identity,
                      gradle_provenance, parse_boolean, parse_dex_packages, parse_integer,
                      parse_signer_digests, parse_summary, read_entries, size_units, summarize_entries)

DEX_OUTPUT = "\n".join([
    "P d 100\t120\t1000\t<TOTAL>",
    "P d 80\t90\t800\tandroidx",
    "P d 60\t60\t600\tandroidx.compose.material.icons",
    "P d 10\t10\t99\tandroidx.compose.material.icons.filled.deep.deeper.deepest",
    "C d 3\t3\t50\tandroidx.compose.material.icons.filled.AccessibleKt",
    "M d 1\t1\t10\tandroidx.compose.material.icons.filled.AccessibleKt getAccessible()",
])


def analyzer(**overrides):
    value = {"status": "measured", "tools": {"apkanalyzer": "apkanalyzer"},
             "identity": {"application_id": "com.example.demo", "version_code": 1, "version_name": "0.1"},
             "file_size_bytes": 900, "download_size_bytes": 800, "debuggable": True,
             "dex": parse_dex_packages(DEX_OUTPUT), "problems": []}
    value.update(overrides)
    return value


def make_report(entries, file_bytes, **kwargs):
    identity = {"sha256": "ab" * 32, "file_bytes": file_bytes}
    provenance = {"variant": "debug", "gradle_task": "unknown", "source_revision": "unknown",
                  "source_dirty": "unknown", "origin": "test"}
    return build_report(identity, summarize_entries(entries, file_bytes), kwargs.get("analyzer", analyzer()),
                        {"status": "NOT_CHECKED", "signer_sha256": None}, provenance, "2026-01-01T00:00:00+00:00")


def entry(name, compressed, uncompressed=None):
    return {"name": name, "compressed_bytes": compressed, "uncompressed_bytes": uncompressed or compressed}


class ClassifyTests(unittest.TestCase):
    def test_groups_every_entry_exactly_once_and_splits_fonts_and_abis(self):
        self.assertEqual(classify("classes2.dex"), ("dex", None))
        self.assertEqual(classify("lib/arm64-v8a/libx.so"), ("native_libs", "arm64-v8a"))
        self.assertEqual(classify("assets/composeResources/font/Inter.ttf"), ("fonts", None))
        self.assertEqual(classify("res/font/inter.otf"), ("fonts", None))
        self.assertEqual(classify("res/drawable/a.xml"), ("res", None))
        self.assertEqual(classify("assets/a.json"), ("assets", None))
        self.assertEqual(classify("META-INF/CERT.RSA"), ("signature_and_meta_inf", None))
        self.assertEqual(classify("kotlin/kotlin.kotlin_builtins"), ("other", None))
        self.assertEqual(classify("sub/classes.dex"), ("other", None))

    def test_totals_never_hide_overhead_and_abis_may_be_empty(self):
        entries = [entry("classes.dex", 700, 2000), entry("assets/a", 100), entry("lib/x86_64/a.so", 50)]
        summary = summarize_entries(entries, 1000)
        self.assertEqual(summary["entry_compressed_bytes"], 850)
        self.assertEqual(summary["container_overhead_bytes"], 150)
        self.assertEqual(sum(group["compressed_bytes"] for group in summary["groups"].values()), 850)
        self.assertEqual(summary["native_libs_by_abi"]["x86_64"]["files"], 1)
        self.assertEqual(summarize_entries([entry("classes.dex", 1)], 10)["native_libs_by_abi"], {})
        self.assertEqual(summary["top_entries"][0]["name"], "classes.dex")


class ParserTests(unittest.TestCase):
    def test_dex_packages_report_notable_prefixes_and_ignore_deep_rows(self):
        parsed = parse_dex_packages(DEX_OUTPUT)
        self.assertEqual(parsed["total"]["estimated_dex_bytes"], 1000)
        icons = parsed["notable"]["androidx.compose.material.icons"]
        self.assertEqual((icons["estimated_dex_bytes"], icons["share_of_dex_estimate"]), (600, 0.6))
        self.assertIsNone(parsed["notable"]["kotlin"])
        names = [row["package"] for row in parsed["top_packages_inclusive"]]
        self.assertNotIn("androidx.compose.material.icons.filled.deep.deeper.deepest", names)

    def test_dex_packages_without_total_or_with_bad_rows_fail(self):
        with self.assertRaises(ToolFailure):
            parse_dex_packages("P d 1\t1\t10\tcom")
        with self.assertRaises(ToolFailure):
            parse_dex_packages("P d 100\t120\t1000\t<TOTAL>\nP d x\t1\t2\tcom")
        with self.assertRaises(ToolFailure):
            parse_dex_packages("")

    def test_scalar_parsers_reject_garbage(self):
        self.assertEqual(parse_integer("123\n", "x"), 123)
        for bad in ("", "1.5", "-3", "abc 3"):
            with self.assertRaises(ToolFailure):
                parse_integer(bad, "x")
        self.assertTrue(parse_boolean("True\n", "x"))
        with self.assertRaises(ToolFailure):
            parse_boolean("maybe", "x")
        self.assertEqual(parse_summary("com.a\t3\t1.0-demo\n")["version_code"], 3)
        with self.assertRaises(ToolFailure):
            parse_summary("only one field")

    def test_signer_digests_are_deduplicated_and_lowercased(self):
        digest = "AB" * 32
        text = f"Signer #1 certificate SHA-256 digest: {digest}\nSigner #1 certificate SHA-256 digest: {digest}\n"
        self.assertEqual(parse_signer_digests(text), [digest.lower()])
        self.assertEqual(parse_signer_digests("Signer #1 certificate SHA-256 digest: short"), [])

    def test_size_units_keep_decimal_and_binary_apart(self):
        self.assertEqual(size_units(1_048_576), {"bytes": 1048576, "decimal_mb": 1.049, "mib": 1.0})


class ReportAndCompareTests(unittest.TestCase):
    def test_missing_or_failed_analyzer_is_never_reported_as_measured(self):
        entries = [entry("classes.dex", 10)]
        for status in ("UNSUPPORTED", "FAILED"):
            report = make_report(entries, 20, analyzer={"status": status, "tools": {"apkanalyzer": None},
                                                         "problems": ["no tool"]})
            self.assertEqual(report["status"], "PARTIAL")
            self.assertIsNone(report["metrics"]["analyzer_estimated_download_bytes"])
            self.assertEqual(report["metrics"]["apk_file_bytes"], 20)
        self.assertEqual(make_report(entries, 20)["status"], "MEASURED")

    def test_delta_is_absolute_and_percent_and_flags_every_input_difference(self):
        before = make_report([entry("classes.dex", 600), entry("assets/a", 100)], 800)
        smaller = analyzer(debuggable=False, dex=parse_dex_packages(DEX_OUTPUT.replace("600\tandroidx.compose.material.icons", "100\tandroidx.compose.material.icons", 1)))
        after = make_report([entry("classes.dex", 300), entry("assets/a", 100)], 450, analyzer=smaller)
        comparison = compare_reports(before, after)
        self.assertEqual(comparison["metrics"]["apk_file_bytes"],
                         {"before": 800, "after": 450, "delta_bytes": -350, "delta_percent": -43.75})
        self.assertEqual(comparison["group_compressed_bytes"]["dex"]["delta_bytes"], -300)
        self.assertEqual(comparison["group_compressed_bytes"]["assets"]["delta_bytes"], 0)
        self.assertEqual(comparison["material_icons_dex_estimate_bytes"]["delta_bytes"], -500)
        self.assertEqual(comparison["input_differences"], {"debuggable": {"before": True, "after": False}})
        self.assertFalse(comparison["comparable"])

    def test_identical_inputs_are_comparable_and_zero_baseline_has_no_percent(self):
        report = make_report([entry("classes.dex", 10)], 20)
        comparison = compare_reports(report, report)
        self.assertTrue(comparison["comparable"])
        self.assertEqual(comparison["metrics"]["apk_file_bytes"]["delta_bytes"], 0)
        zero = apk_size._delta(0, 5)
        self.assertIsNone(zero["delta_percent"])
        self.assertIsNone(apk_size._delta(None, 5))

    def test_a_side_without_analyzer_metrics_yields_not_comparable_not_zero(self):
        partial = make_report([entry("classes.dex", 10)], 20,
                              analyzer={"status": "UNSUPPORTED", "tools": {"apkanalyzer": None}, "problems": []})
        comparison = compare_reports(make_report([entry("classes.dex", 10)], 20), partial)
        self.assertIsNone(comparison["metrics"]["analyzer_estimated_download_bytes"])
        self.assertIsNone(comparison["material_icons_dex_estimate_bytes"])
        self.assertIn("analyzer_version", comparison["input_differences"])

    def test_unknown_schema_is_rejected(self):
        report = make_report([entry("classes.dex", 10)], 20)
        with self.assertRaises(ValueError):
            compare_reports(dict(report, schema_version=2), report)


class AdapterTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.dir = Path(self.temporary.name)

    def make_apk(self, name="app-debug.apk"):
        path = self.dir / name
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
            archive.writestr("classes.dex", b"dex" * 100)
            archive.writestr("assets/a.txt", b"hello")
            archive.writestr("lib/arm64-v8a/liba.so", b"\x7fELF")
        return path

    def test_missing_empty_and_corrupt_apks_fail_with_a_reason(self):
        with self.assertRaises(ValueError):
            file_identity(self.dir / "nope.apk")
        empty = self.dir / "empty.apk"
        empty.write_bytes(b"")
        with self.assertRaises(ValueError):
            file_identity(empty)
        corrupt = self.dir / "corrupt.apk"
        corrupt.write_bytes(b"not a zip at all")
        with self.assertRaisesRegex(ValueError, "not a readable ZIP"):
            read_entries(corrupt)
        with self.assertRaises(ValueError):
            file_identity(self.dir)

    def test_reads_entries_and_identity_of_a_real_zip(self):
        apk = self.make_apk()
        names = sorted(item["name"] for item in read_entries(apk))
        self.assertEqual(names, ["assets/a.txt", "classes.dex", "lib/arm64-v8a/liba.so"])
        identity = file_identity(apk)
        self.assertEqual(identity["file_bytes"], apk.stat().st_size)
        self.assertEqual(len(identity["sha256"]), 64)

    def test_provenance_is_unknown_without_matching_gradle_metadata(self):
        apk = self.make_apk()
        self.assertEqual(gradle_provenance(apk, None)["variant"], "unknown")
        (self.dir / "output-metadata.json").write_text(json.dumps(
            {"variantName": "release", "elements": [{"outputFile": "other.apk"}]}))
        self.assertEqual(gradle_provenance(apk, None)["variant"], "unknown")
        (self.dir / "output-metadata.json").write_text(json.dumps(
            {"variantName": "debug", "elements": [{"outputFile": apk.name}]}))
        provenance = gradle_provenance(apk, "abc123")
        self.assertEqual((provenance["variant"], provenance["source_revision"]), ("debug", "abc123"))
        (self.dir / "output-metadata.json").write_text("{broken")
        self.assertEqual(gradle_provenance(apk, None)["variant"], "unknown")

    def test_missing_analyzer_writes_a_partial_report_and_fails_closed(self):
        apk = self.make_apk()
        with patch.object(apk_size, "find_android_tool", return_value=None), \
                patch.object(apk_size, "OUTPUT_ROOT", self.dir / "artifacts/apk-size"):
            output = self.dir / "artifacts/apk-size/run"
            self.assertEqual(apk_size.run(apk, output, None, None), 1)
            report = json.loads((output / "report.json").read_text())
            self.assertEqual(report["status"], "PARTIAL")
            self.assertEqual(report["analyzer_status"], "UNSUPPORTED")
            self.assertEqual(report["signing"]["status"], "NOT_CHECKED")
            self.assertEqual(report["zip"]["groups"]["dex"]["files"], 1)
            self.assertEqual(report["provenance"]["source_revision"], "unknown")

    def test_output_must_be_fresh_and_beneath_the_artifact_tree(self):
        apk = self.make_apk()
        with patch.object(apk_size, "OUTPUT_ROOT", self.dir / "artifacts/apk-size"):
            with self.assertRaisesRegex(ValueError, "beneath artifacts/apk-size"):
                apk_size.run(apk, self.dir / "elsewhere", None, None)
            existing = self.dir / "artifacts/apk-size/old"
            existing.mkdir(parents=True)
            with self.assertRaisesRegex(ValueError, "never overwritten"):
                apk_size.run(apk, existing, None, None)

    def test_tool_failure_is_a_failed_analyzer_not_a_pass(self):
        apk = self.make_apk()
        with patch.object(apk_size, "find_android_tool", return_value="/bin/fake"), \
                patch.object(apk_size, "run_tool", side_effect=ToolFailure("boom")):
            result = apk_size.run_analyzer(apk)
        self.assertEqual(result["status"], "FAILED")
        self.assertEqual(len(result["problems"]), 5)

    def test_baseline_is_validated_before_any_output_exists(self):
        apk = self.make_apk()
        bad = self.dir / "baseline.json"
        bad.write_text("{nope")
        with patch.object(apk_size, "OUTPUT_ROOT", self.dir / "artifacts/apk-size"):
            with self.assertRaisesRegex(ValueError, "baseline report is unreadable"):
                apk_size.run(apk, self.dir / "artifacts/apk-size/run", bad, None)
            self.assertFalse((self.dir / "artifacts/apk-size/run").exists())


if __name__ == "__main__":
    unittest.main()
