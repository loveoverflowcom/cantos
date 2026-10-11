"""Independent-oracle arithmetic/transport guards; doubles do not prove PostgreSQL."""

import copy
import hashlib
import json
import subprocess
import unittest
from unittest.mock import MagicMock, patch

import production_oracle as oracle


class ProductionOracleTests(unittest.TestCase):
    def test_anonymous_error_receipt_is_not_misclassified_as_a_leaked_empty_token(self):
        response = MagicMock()
        response.__enter__.return_value = response
        response.status = 401
        response.headers = {"Cache-Control": "no-store", "X-Content-Type-Options": "nosniff"}
        receipt = {"code": "unauthenticated", "current_revision": None, "issues": []}
        response.read.return_value = json.dumps(receipt).encode()
        opener = MagicMock()
        opener.open.return_value = response
        with patch.object(oracle.urllib.request, "build_opener", return_value=opener), \
                patch.object(oracle.urllib.request, "ProxyHandler") as proxy:
            self.assertEqual(oracle.http(12345, "", "/api/v1/production/catalog", expected_status=401), receipt)
            proxy.assert_called_once_with({})
        response.status = 200
        response.read.return_value = b'{"code":"synthetic-session-must-be-suppressed"}'
        with patch.object(oracle.urllib.request, "build_opener", return_value=opener), \
                self.assertRaisesRegex(RuntimeError, "output suppressed") as failure:
            oracle.http(12345, "synthetic-session-must-be-suppressed", "/api/v1/production/catalog")
        self.assertNotIn("synthetic-session-must-be-suppressed", str(failure.exception))

    def test_small_latency_sample_reports_the_outlier_without_a_tail_or_throughput_claim(self):
        result = oracle.summarize_latency([1, 2, 3, 4, 5, 6, 7, 8, 123])
        self.assertEqual(result["p95_ms"], 123)
        self.assertEqual(result["median_ms"], 5)
        self.assertFalse(result["local_target_met"])
        self.assertNotIn("p99_ms", result)
        self.assertNotIn("requests_per_second", result)

    def test_scope_projection_sorts_declared_sets_and_excludes_unused_claims_without_mutation(self):
        script = {"work": {"rights_record_id": "rights-work"},
                  "adaptation": {"rights_record_id": "rights-adaptation"},
                  "provenance": [], "episode": {"acts": []}}
        revision = {"script_json": json.dumps(script)}
        settings = {"settings": {"bindings": [
            {"character_id": "z", "voice_rights_record_id": "rights-voice", "pronunciation": [
                {"surface": "z", "replacement": "z"}, {"surface": "a", "replacement": "a"}]},
            {"character_id": "a", "voice_rights_record_id": "rights-voice", "pronunciation": []}]}}
        rights = [{"claim": {"declaration": {"record_id": record_id}}}
                  for record_id in ["unused", "rights-work", "rights-voice", "rights-adaptation"]]
        original = copy.deepcopy((revision, settings, rights))
        projected = oracle.scoped_projection(revision, settings, rights,
                                             {"resolved": [], "estimate": {}}, "catalog")
        self.assertEqual([item["character_id"] for item in projected["settings"]["settings"]["bindings"]], ["a", "z"])
        self.assertEqual([item["surface"] for item in projected["settings"]["settings"]["bindings"][1]["pronunciation"]], ["a", "z"])
        self.assertEqual([item["claim"]["declaration"]["record_id"] for item in projected["rights"]],
                         ["rights-adaptation", "rights-voice", "rights-work"])
        self.assertEqual((revision, settings, rights), original)

    def test_resolution_uses_longest_original_surface_once_and_preserves_delivery(self):
        binding = {"character_id": "an", "provider_id": "reference-only", "model_id": "reference-v1",
                   "voice_id": "synthetic-character", "voice_rights_record_id": "rights-test", "language": "vi-VN",
                   "performance": {"rate_permille": 1000, "pitch_semitones": 0, "emotion": None,
                                   "intensity_permille": None}, "pronunciation": [
                       {"surface": "Vọng", "replacement": "wrong"},
                       {"surface": "Vọng Đài", "replacement": "Vọng"}]}
        line = {"id": "line", "speaker_id": "an", "text": "Vọng Đài 🎭",
                "delivery": {"emotion": "hopeful", "intensity_permille": 600}}
        script = {"episode": {"acts": [{"scenes": [{"dialogues": [line]}]}]}}
        result = oracle.resolved_inputs(script, {"bindings": [binding]})[0]
        self.assertEqual(result["text"], "Vọng 🎭")
        self.assertEqual(result["performance"], {"rate_permille": 1000, "pitch_semitones": 0,
                                               "emotion": "hopeful", "intensity_permille": 600})
        self.assertEqual(result["output_contract"], "reference-only/no-audio")
        line["pronunciation_overrides"] = [{"surface": "Vọng Đài", "replacement": "conflict"}]
        with self.assertRaisesRegex(ValueError, "conflicting pronunciation"):
            oracle.resolved_inputs(script, {"bindings": [binding]})

    def test_scoped_digest_uses_utf8_sorted_compact_json_and_explicit_tag(self):
        document = {"owner_id": "alice", "text": 'Vọng Đài 🎭 "\\',
                    "nested": {"z": 2, "a": None}, "input_digest": "ignored"}
        original = copy.deepcopy(document)
        expected_bytes = '{"nested":{"a":null,"z":2},"owner_id":"alice","text":"Vọng Đài 🎭 \\\"\\\\"}'.encode()
        expected = "production-p1:sha256:" + hashlib.sha256(
            b"cantos/production-inputs/p1\n" + expected_bytes).hexdigest()
        self.assertEqual(oracle.input_digest(document), expected)
        self.assertEqual(document, original)
        document["input_digest"] = "another supplied digest"
        self.assertEqual(oracle.input_digest(document), expected)
        document["owner_id"] = "bob"
        self.assertNotEqual(oracle.input_digest(document), expected)

    def test_rights_and_budget_are_digest_inputs_even_if_content_digest_is_unchanged(self):
        document = {"revision": {"content_digest": "same-content"},
                    "rights": [{"version": 1, "status": "granted"}],
                    "settings": {"budget": {"limit_minor": 7}}}
        baseline = oracle.input_digest(document)
        for changed in ({"rights": [{"version": 2, "status": "revoked"}]},
                        {"settings": {"budget": {"limit_minor": 8}}}):
            self.assertNotEqual(oracle.input_digest(dict(document, **changed)), baseline)

    def test_unicode_scalars_and_exact_integer_ceiling_are_separate_from_bytes(self):
        nfc = "Ngày mai, mình có diễn tiếp không?"
        nfd = "Nga\u0300y mai, mi\u0300nh co\u0301 die\u0302\u0303n tie\u0302\u0301p kho\u0302ng?"
        self.assertNotEqual(len(nfc), len(nfc.encode()))
        self.assertNotEqual(len(nfc), len(nfd))
        rate = {"units_per_charge": 7, "amount_minor": 3}
        texts = [nfc, "🎭", nfd]
        units, amount = oracle.estimate(texts, rate)
        self.assertEqual(units, len(nfc) + 1 + len(nfd))
        self.assertEqual(amount, sum(-(-(len(text) * 3) // 7) for text in texts))
        self.assertEqual(oracle.estimate(["a", "b", "c"], rate), (3, 3))
        self.assertEqual(oracle.estimate(["abc"], rate), (3, 2))
        for count in range(22):
            with self.subTest(count=count):
                self.assertEqual(oracle.estimate(["x" * count], rate),
                                 (count, -(-(count * 3) // 7)))

    def test_estimate_never_rounds_down_near_integer_bounds(self):
        rate = {"units_per_charge": 3, "amount_minor": oracle.I64_MAX}
        self.assertEqual(oracle.estimate(["ab"], rate),
                         (2, -(-(2 * oracle.I64_MAX) // 3)))
        with self.assertRaisesRegex(ValueError, "minor-unit bound"):
            oracle.estimate(["abcd"], rate)
        with self.assertRaisesRegex(ValueError, "minor-unit bound"):
            oracle.estimate(["a", "b", "c"], rate)
        for invalid in ({"units_per_charge": 0, "amount_minor": 1},
                        {"units_per_charge": 1, "amount_minor": -1},
                        {"units_per_charge": oracle.I64_MAX + 1, "amount_minor": 1},
                        {"units_per_charge": 1, "amount_minor": oracle.I64_MAX + 1},
                        {"units_per_charge": True, "amount_minor": 1}):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                oracle.estimate(["a"], invalid)
        with self.assertRaises(UnicodeEncodeError):
            oracle.estimate(["\ud800"], {"units_per_charge": 1, "amount_minor": 1})

    def test_sql_only_accepts_the_new_disposable_app_role_database_and_fails_closed(self):
        url = "postgresql://cantos_app@127.0.0.1:12345/cantos_test_synthetic"
        completed = subprocess.CompletedProcess([], 0, '{"snapshots":1}', "")
        with patch.object(oracle.subprocess, "run", return_value=completed) as execute:
            self.assertEqual(oracle.sql({"PATH": "/synthetic"}, url, "SELECT 1"),
                             {"snapshots": 1})
        self.assertIn(url, execute.call_args.args[0])
        self.assertIn("ON_ERROR_STOP=1", execute.call_args.args[0])
        self.assertEqual(execute.call_args.kwargs["timeout"], 10)
        for rejected in (url.replace("127.0.0.1", "example.invalid"),
                         url.replace("cantos_test_synthetic", "postgres"),
                         url.replace("cantos_app@", "cantos_test_admin@"),
                         url + "?options=-csearch_path=private",
                         url.replace("cantos_app@", "cantos_app:secret@")):
            with self.subTest(url=rejected), self.assertRaises(ValueError), \
                    patch.object(oracle.subprocess, "run") as execute:
                oracle.sql({}, rejected, "SELECT 1")
                execute.assert_not_called()
        failed = subprocess.CompletedProcess([], 1, "", "synthetic failure")
        with patch.object(oracle.subprocess, "run", return_value=failed), \
                self.assertRaisesRegex(RuntimeError, "app-role SQL oracle failed"):
            oracle.sql({}, url, "SELECT 1")

    def test_input_only_journey_checks_zero_execution_rows_after_durable_migration(self):
        script = "8f8ac1f2-4bc3-4c80-808e-ac02a62917b2"
        counts = {"generation_attempts": 0, "generation_or_cost_rows": 0}
        with patch.object(oracle, "sql", return_value=counts) as execute:
            self.assertEqual(oracle.row_counts({}, "local-test-url", script), counts)
        query = execute.call_args.args[2]
        for table in ("durable_attempts", "durable_reservations", "durable_settlements"):
            self.assertIn(f"count(*) FROM {table}", query)
        self.assertNotIn("pg_tables", query)
        oracle.assert_delta(counts, counts)
        with self.assertRaises(AssertionError):
            oracle.assert_delta(counts, dict(counts, generation_or_cost_rows=1))

    def test_http_fails_before_network_for_an_invalid_port_or_non_api_path(self):
        for port, path in ((0, "/api/v1/production/catalog"),
                           (65536, "/api/v1/production/catalog"),
                           (12345, "https://example.invalid"),
                           (12345, "/api/v1/production/catalog\r\nInjected: yes")):
            with self.subTest(port=port, path=path), self.assertRaises(ValueError), \
                    patch.object(oracle.urllib.request, "build_opener") as opener:
                oracle.http(port, "synthetic", path)
                opener.assert_not_called()


if __name__ == "__main__":
    unittest.main()
