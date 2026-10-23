"""Exact-job Pages orchestration on real v2 artifacts, with the publisher command doubled."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_pipeline as release
from test_release_artifact import ENV, SHA, fixture, request

PUBLISHER_ENV = {**ENV, "CIRCLE_WORKFLOW_JOB_ID": "publisher-job", "CIRCLE_BUILD_NUM": "456",
                 "CLOUDFLARE_ACCOUNT_ID": "a" * 32, "CLOUDFLARE_THEATRE_PROJECT": "fixture-theatre",
                 "CLOUDFLARE_STUDIO_PROJECT": "fixture-studio", "CLOUDFLARE_PRODUCTION_BRANCH": "main",
                 "WEB_ENVIRONMENT": "preview", "WEB_BRANCH": "review-pages"}


@patch.dict(os.environ, PUBLISHER_ENV, clear=True)
class PagesPromotion(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.transfer = self.root / "transfer"
        self.transfer.mkdir()
        self.request = request()
        self.manifest, _ = fixture(self.transfer, self.request)
        self.digest = contract.file_record(self.manifest, self.manifest.parent)["sha256"]
        contract.write_new(self.transfer / "request.json", self.request)
        contract.write_new(self.transfer / "inventory.json", {"schema_version": 1,
            "request_sha256": contract.fingerprint(self.request), "components": {"theatre-web": {
                "manifest": self.manifest.relative_to(self.transfer).as_posix(), "digest": self.digest}}})
        self.source = patch.object(release.release_build, "source_revision", return_value=SHA)
        self.fresh = patch.object(release, "assert_fresh")
        self.source.start()
        self.fresh.start()
        self.addCleanup(self.source.stop)
        self.addCleanup(self.fresh.stop)

    def intent(self):
        with patch.object(release, "run") as run:
            release.pages_intent(self.root, self.transfer, "theatre-web")
            return run.call_args.args[1]

    def receipt(self, *, smoke="pass"):
        return {"app": "cantos-theatre", "account": "a" * 32, "project": "fixture-theatre",
                "branch": "review-pages", "environment": "preview",
                "artifact": {"source_sha": SHA, "manifest_sha256": self.digest},
                "remote": {"state": "confirmed", "release": {"deployment_id": "exact-id"}},
                "smoke": None if smoke is None else {"state": smoke}}

    def test_success_requires_a_matching_receipt_and_passed_smoke(self):
        self.intent()
        def publisher(root, argv):
            path = root / "artifacts/cloudflare/attempt/receipt.json"
            path.parent.mkdir(parents=True)
            contract.write_new(path, self.receipt())
        with patch.object(release, "run", side_effect=publisher) as run:
            release.pages_promote(self.root, self.transfer, "theatre-web")
            self.assertEqual(run.call_count, 1)
        record = contract.read_json(self.transfer / "outcomes/theatre-web/outcome.json")
        self.assertEqual((record["outcome"], record["smoke"], record["command_status"]), ("confirmed", "PASS", "PASS"))

    def test_exit_zero_without_a_receipt_is_unknown_and_fails(self):
        self.intent()
        with patch.object(release, "run"):
            with self.assertRaisesRegex(ValueError, "unconfirmed"):
                release.pages_promote(self.root, self.transfer, "theatre-web")
        record = contract.read_json(self.transfer / "outcomes/theatre-web/outcome.json")
        self.assertEqual(record["outcome"], "unknown")

    def test_foreign_app_or_artifact_receipt_cannot_confirm_this_attempt(self):
        self.intent()
        def publisher(root, argv):
            path = root / "artifacts/cloudflare/foreign/receipt.json"
            path.parent.mkdir(parents=True)
            receipt = self.receipt()
            receipt["app"] = "cantos-studio"
            receipt["artifact"]["source_sha"] = "f" * 40
            contract.write_new(path, receipt)
        with patch.object(release, "run", side_effect=publisher):
            with self.assertRaisesRegex(ValueError, "unconfirmed"):
                release.pages_promote(self.root, self.transfer, "theatre-web")
        record = contract.read_json(self.transfer / "outcomes/theatre-web/outcome.json")
        self.assertEqual(record["receipt_status"], "FOREIGN_OR_INCOMPLETE")
        self.assertEqual(record["outcome"], "unknown")

    def test_intent_binds_exact_source_destination_and_uses_only_existing_deployer(self):
        argv = self.intent()
        self.assertEqual(argv[:6], ["cargo", "xtask", "deploy", "cloudflare", "cantos-theatre", "--web"])
        self.assertEqual(argv[-1], "--dry-run")
        planned = contract.read_json(self.transfer / "intents/theatre-web/intent.json")
        self.assertEqual(planned["manifest_sha256"], self.digest)
        self.assertEqual(planned["destination"], {"account": "a" * 32, "project": "fixture-theatre",
                                                "branch": "review-pages", "environment": "preview"})
        self.assertNotIn("TOKEN", json.dumps(planned))
        with self.assertRaisesRegex(ValueError, "intent already exists"):
            self.intent()

    def test_mapping_drift_wrong_component_corruption_and_staleness_block_before_intent(self):
        self.intent()
        mapping = self.root / "infra/cloudflare/projects.json"
        mapping.write_text('{"different":true}')
        other = self.root / "other-transfer"
        with patch.object(release, "promotion_input", return_value=(None, self.request, {"digest": self.digest}, self.manifest)):
            with self.assertRaisesRegex(ValueError, "mapping differs"):
                release.pages_intent(self.root, other, "theatre-web")
        self.assertFalse(other.exists())
        with self.assertRaisesRegex(ValueError, "not approved"):
            release.pages_intent(self.root, self.transfer, "studio-web")
        self.manifest.write_bytes(b"corrupt")
        with self.assertRaisesRegex(ValueError, "manifest digest mismatch"):
            release.pages_promote(self.root, self.transfer, "theatre-web")

    def test_rerun_job_refuses_and_lost_response_preserves_confirmed_remote_identity(self):
        self.intent()
        with patch.dict(os.environ, {"CIRCLE_BUILD_NUM": "457"}), patch.object(release, "run") as run:
            with self.assertRaisesRegex(ValueError, "rerun Pages intent"):
                release.pages_promote(self.root, self.transfer, "theatre-web")
            run.assert_not_called()
        def publish_then_lose(root, argv):
            self.assertNotIn("--dry-run", argv)
            path = root / "artifacts/cloudflare/attempt/receipt.json"
            path.parent.mkdir(parents=True)
            contract.write_new(path, self.receipt(smoke=None))
            raise subprocess.TimeoutExpired(argv, 1)
        with patch.object(release, "run", side_effect=publish_then_lose) as run:
            with self.assertRaises(subprocess.TimeoutExpired):
                release.pages_promote(self.root, self.transfer, "theatre-web")
            self.assertEqual(run.call_count, 1)
        record = contract.read_json(self.transfer / "outcomes/theatre-web/outcome.json")
        self.assertEqual(record["outcome"], "confirmed")
        self.assertEqual(record["remote_identity"]["release"]["deployment_id"], "exact-id")
        self.assertEqual(record["command_status"], "FAIL")
        self.assertEqual(record["smoke"], "NOT_RUN")
        with patch.object(release, "run") as run:
            with self.assertRaisesRegex(ValueError, "already attempted"):
                release.pages_promote(self.root, self.transfer, "theatre-web")
            run.assert_not_called()

    def test_unknown_publish_does_not_become_success_or_retry(self):
        self.intent()
        with patch.object(release, "run", side_effect=OSError("command unavailable")):
            with self.assertRaises(OSError):
                release.pages_promote(self.root, self.transfer, "theatre-web")
        record = contract.read_json(self.transfer / "outcomes/theatre-web/outcome.json")
        self.assertEqual(record["outcome"], "unknown")
        self.assertIsNone(record["remote_identity"])
        self.assertEqual(record["command_status"], "FAIL")


if __name__ == "__main__":
    unittest.main()
