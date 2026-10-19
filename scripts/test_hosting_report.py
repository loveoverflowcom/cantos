"""Synthetic local records only; no account/provider/HTTP/browser acceptance."""

import contextlib
import copy
import io
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import hosting_report as report


TEMPLATE = Path(__file__).resolve().parents[1] / "templates/hosting-rollout-record.json"
SOURCE = "a" * 40
CONFIG = "b" * 40
DIGEST = "1" * 64
SENTINEL = "synthetic-private-sentinel"


def record():
    value = json.loads(TEMPLATE.read_text())
    value["release_sha"] = SOURCE
    value["config_sha"] = CONFIG
    value["approved_api_origin"] = "https://api.example.org"
    value["circleci"] = {"pipeline_id": "synthetic-pipeline", "workflow_id": "synthetic-workflow",
                         "job_number": "123"}
    for name, component in value["components"].items():
        component.update(artifact_sha256=DIGEST, api_origin="https://api.example.org",
                         deployment_id="synthetic-new", deployment_environment="production",
                         deployment_origin="https://synthetic.pages.dev", final_origin=report.COMPONENTS[name],
                         rollback_deployment_id="synthetic-old", rollback_environment="production")
    return value


def passed(kind="production-observed"):
    return {"status": "PASS", "kind": kind, "source_sha": SOURCE, "evidence_sha256": [DIGEST]}


class HostingReportTests(unittest.TestCase):
    def assert_reason(self, value, reason):
        with self.assertRaises(report.RecordError) as caught:
            report.validate(value)
        self.assertEqual(str(caught.exception), reason)

    def test_template_never_implies_configured_accounts_or_completed_rollout(self):
        value = report.load(TEMPLATE)
        self.assertIsNone(value["release_sha"])
        self.assertIsNone(value["config_sha"])
        self.assertIsNone(value["approved_api_origin"])
        text = report.render(value)
        self.assertEqual(text.count("DECLARED NOT_RUN"), 27)
        self.assertIn("27 of 27 rows lack a declared PASS", text)
        self.assertIn("live state were NOT verified", text)

    def test_publish_survives_failed_smoke_and_other_app_stays_unrun(self):
        value = record()
        component = value["components"]["theatre-web"]
        component["checks"]["cloudflare_publish"] = passed()
        component["checks"]["preview_http"]["status"] = "FAIL"
        snapshot = copy.deepcopy(value)
        text = report.render(value)
        self.assertIn("DECLARED PASS    theatre-web/cloudflare_publish", text)
        self.assertIn("DECLARED FAIL    theatre-web/preview_http", text)
        self.assertIn("DECLARED NOT_RUN studio-web/cloudflare_publish", text)
        self.assertEqual(value, snapshot)

    def test_unknown_outcome_keeps_successful_component_and_demands_reconciliation(self):
        value = record()
        value["components"]["theatre-web"]["checks"]["cloudflare_publish"] = passed()
        value["components"]["studio-web"]["checks"]["cloudflare_publish"]["status"] = "UNKNOWN"
        text = report.render(value)
        self.assertIn("DECLARED PASS    theatre-web/cloudflare_publish", text)
        self.assertIn("DECLARED UNKNOWN studio-web/cloudflare_publish", text)
        self.assertIn("before any retry", text)
        self.assertIn("26 of 27 rows lack a declared PASS", text)

    def test_every_status_round_trips_without_promotion_to_pass(self):
        for status in report.STATUSES:
            value = record()
            value["checks"]["operator_scope"] = passed("documented")
            value["checks"]["operator_scope"]["status"] = status
            self.assertIn(f"DECLARED {status:7} global/operator_scope", report.render(value))

    def test_mock_cannot_satisfy_any_live_observation(self):
        for name in report.LIVE_CHECKS:
            value = record()
            checks = (value["checks"] if name in report.GLOBAL_CHECKS else
                      value["components"]["theatre-web"]["checks"])
            checks[name] = passed("mock-integration-tested")
            self.assert_reason(value, "LIVE_EVIDENCE_REQUIRED")
        for name in ("preview_browser", "final_browser"):
            value = record()
            value["components"]["theatre-web"]["checks"][name] = passed()
            self.assert_reason(value, "BROWSER_EVIDENCE_REQUIRED")

    def test_pass_requires_evidence_source_config_artifact_and_remote_identity(self):
        for field, reason in (("kind", "PASS_WITHOUT_EVIDENCE"),
                              ("source_sha", "SOURCE_MISMATCH")):
            value = record()
            value["checks"]["circleci_build"] = passed()
            value["checks"]["circleci_build"][field] = None
            self.assert_reason(value, reason)
        value = record()
        value["checks"]["circleci_build"] = passed()
        value["checks"]["circleci_build"]["evidence_sha256"] = []
        self.assert_reason(value, "PASS_WITHOUT_EVIDENCE")
        for field in ("config_sha",):
            value = record()
            value["checks"]["circleci_build"] = passed()
            value[field] = None
            self.assert_reason(value, "CI_IDENTITY_REQUIRED")
        for field in ("pipeline_id", "workflow_id", "job_number"):
            value = record()
            value["checks"]["circleci_build"] = passed()
            value["circleci"][field] = None
            self.assert_reason(value, "CI_IDENTITY_REQUIRED")
        for field in ("artifact_sha256", "api_origin", "deployment_id", "deployment_origin",
                      "deployment_environment"):
            value = record()
            component = value["components"]["theatre-web"]
            component["checks"]["cloudflare_publish"] = passed()
            component[field] = None
            self.assert_reason(value, "ARTIFACT_IDENTITY_REQUIRED" if field in
                               {"artifact_sha256", "api_origin"} else "DEPLOYMENT_IDENTITY_REQUIRED")

    def test_foreign_source_or_api_origin_cannot_be_reported_pass(self):
        value = record()
        value["components"]["theatre-web"]["checks"]["cloudflare_publish"] = passed()
        value["components"]["theatre-web"]["checks"]["cloudflare_publish"]["source_sha"] = "c" * 40
        self.assert_reason(value, "SOURCE_MISMATCH")
        value = record()
        value["components"]["studio-web"]["checks"]["artifact"] = passed("compiled")
        value["components"]["studio-web"]["api_origin"] = "https://other.example.org"
        self.assert_reason(value, "ARTIFACT_IDENTITY_REQUIRED")

    def test_url_parser_cleanup_cannot_turn_a_noncanonical_origin_into_pass(self):
        origin = "https://api.example.org"
        malformed = [chr(code) + origin for code in range(33)]
        malformed.extend(origin + suffix for suffix in ("?", "#", "?#", "\t", "\r", "\n"))
        malformed.extend("https://api." + control + "example.org" for control in ("\t", "\r", "\n"))
        for replacement in malformed:
            with self.subTest(origin=repr(replacement)):
                value = record()
                value["approved_api_origin"] = replacement
                component = value["components"]["theatre-web"]
                component["api_origin"] = replacement
                component["checks"]["artifact"] = passed("compiled")
                snapshot = copy.deepcopy(value)
                self.assert_reason(value, "ORIGIN")
                self.assertEqual(value, snapshot)

    def test_preview_or_wrong_surface_never_satisfies_final_domain_or_rollback(self):
        for check, field, replacement, reason in (
            ("final_http", "deployment_environment", "preview", "FINAL_DOMAIN_REQUIRED"),
            ("final_browser", "final_origin", "https://studio.cantos.cc", "SURFACE_MISMATCH"),
            ("release_rollback", "rollback_environment", "preview", "ROLLBACK_TARGET_REQUIRED"),
            ("release_rollback", "rollback_deployment_id", "synthetic-new", "ROLLBACK_TARGET_REQUIRED"),
        ):
            value = record()
            component = value["components"]["theatre-web"]
            component["checks"][check] = passed("interaction-tested")
            component[field] = replacement
            self.assert_reason(value, reason)

    def test_all_declared_pass_still_grants_no_authority_or_live_verification(self):
        value = record()
        for checks in (value["checks"], *(c["checks"] for c in value["components"].values())):
            for name in checks:
                checks[name] = passed("interaction-tested")
        text = report.render(value)
        self.assertIn("0 of 27 rows lack a declared PASS", text)
        self.assertIn("authorizes no operation or retirement", text)
        self.assertIn("live state were NOT verified", text)

    def test_invalid_schema_versions_fields_types_statuses_and_urls_fail_safely(self):
        for field, replacement, reason in (
            ("schema_version", True, "VERSION"), ("schema_version", 2, "VERSION"),
            ("release_sha", "short", "DIGEST"), ("checks", [], "FIELDS"),
            ("approved_api_origin", "https://api.example.org/?oobCode=" + SENTINEL, "ORIGIN"),
            ("approved_api_origin", "https://user:password@api.example.org", "ORIGIN"),
            ("approved_api_origin", "https://api.example.org:443", "ORIGIN"),
            ("approved_api_origin", "http://api.example.org", "ORIGIN"),
        ):
            value = record()
            value[field] = replacement
            self.assert_reason(value, reason)
        for field in ("status", "kind"):
            value = record()
            value["checks"]["operator_scope"][field] = [SENTINEL]
            self.assert_reason(value, field.upper())
        value = record()
        value[SENTINEL] = SENTINEL
        self.assert_reason(value, "FIELDS")

    def test_reader_is_bounded_and_rejects_duplicate_nonfinite_malformed_and_linked_input(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "record.json"
            for data, reason in ((b"x" * (report.MAX_BYTES + 1), "TOO_LARGE"),
                                 (b'{"schema_version":1,"schema_version":1}', "DUPLICATE_FIELD"),
                                 (b'{"x":NaN}', "JSON"), (b"\xff", "JSON"),
                                 (("[" * 2000).encode(), "JSON"),
                                 (("{\"x\":" + "1" * 5000 + "}").encode(), "JSON")):
                path.write_bytes(data)
                with self.assertRaisesRegex(report.RecordError, "^" + reason + "$"):
                    report.load(path)
            link = Path(directory) / "linked.json"
            link.symlink_to(path)
            with self.assertRaisesRegex(report.RecordError, "^READ_FAILED$"):
                report.load(link)
            with self.assertRaisesRegex(report.RecordError, "^REGULAR_FILE_REQUIRED$"):
                report.load(directory)
            fifo = Path(directory) / "pipe"
            os.mkfifo(fifo)
            with self.assertRaisesRegex(report.RecordError, "^REGULAR_FILE_REQUIRED$"):
                report.load(fifo)

    def test_cli_never_echoes_private_record_values_or_parser_errors(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "record.json"
            for data in (SENTINEL, json.dumps({SENTINEL: SENTINEL}),
                         json.dumps({**record(), "approved_api_origin": "https://a.org/?code=" + SENTINEL})):
                path.write_text(data)
                output = io.StringIO()
                with contextlib.redirect_stdout(output):
                    self.assertEqual(report.main(["--record", str(path)]), 2)
                self.assertNotIn(SENTINEL, output.getvalue())
                self.assertTrue(output.getvalue().startswith("hosting-report: INVALID_RECORD ("))
            value = record()
            value["components"]["theatre-web"]["deployment_id"] = SENTINEL
            path.write_text(json.dumps(value))
            output = io.StringIO()
            with patch("socket.socket", side_effect=AssertionError("network forbidden")), \
                 patch("subprocess.Popen", side_effect=AssertionError("subprocess forbidden")), \
                 contextlib.redirect_stdout(output):
                self.assertEqual(report.main(["--record", str(path)]), 0)
            self.assertNotIn(SENTINEL, output.getvalue())


if __name__ == "__main__":
    unittest.main()
