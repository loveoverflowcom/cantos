"""The speech-fingerprint oracle follows the written contract, not the Rust implementation.

Expected strings are written out by hand from the contract's field rules, so these tests pin
the oracle that pins the Rust encoder.
"""

import hashlib
import unittest

import reference_speech_fingerprint as oracle


class CanonicalDocumentFollowsTheContract(unittest.TestCase):
    def test_base_document_is_sorted_compact_and_free_of_identity_fields(self):
        self.assertEqual(
            oracle.canonical_json(oracle.base_input()),
            '{"adapter_version":"espeak-ng-local-v1","language":"vi-VN",'
            '"model_id":"espeak-ng-1.52.0","output_contract":"wav-pcm-s16le-mono-22050-v1",'
            '"performance":{"emotion":"neutral","intensity_permille":0,'
            '"pitch_semitones":0,"rate_permille":1000},'
            '"pronunciation":[{"replacement":"Ân","surface":"An"},'
            '{"replacement":"Vọng Đài","surface":"Vọng Đài"}],'
            '"provider_id":"espeak-ng-local","text":"Ngày mai, mình có diễn tiếp không?",'
            '"voice_id":"vi-vn-x-south"}',
        )

    def test_fingerprint_is_the_tagged_sha256_of_the_document(self):
        document = oracle.canonical_json(oracle.base_input()).encode("utf-8")
        expected = hashlib.sha256(b"cantos/speech-fingerprint/v1\n" + document).hexdigest()
        self.assertEqual(oracle.fingerprint(oracle.base_input()),
                         f"speech-v1:sha256:{expected}")

    def test_control_characters_use_standard_json_escapes(self):
        document = oracle.canonical_json(oracle.variant(text='a"b\\c\td\u0001'))
        self.assertIn('"text":"a\\"b\\\\c\\td\\u0001"', document)

    def test_absent_emotion_is_null_not_neutral(self):
        document = oracle.canonical_json(oracle.variant(performance__emotion=None))
        self.assertIn('"emotion":null', document)


class SizeBound(unittest.TestCase):
    def test_a_document_beyond_the_registry_bound_has_no_fingerprint(self):
        huge = oracle.variant(pronunciation=[{"surface": "An", "replacement": "a" * (2 * 1024 * 1024)}])
        with self.assertRaises(oracle.Rejected) as caught:
            oracle.fingerprint(huge)
        self.assertEqual(str(caught.exception), "input_too_large")

    def test_just_under_the_bound_is_still_a_valid_request(self):
        large = oracle.variant(pronunciation=[{"surface": "An", "replacement": "a" * (2 * 1024 * 1024 - 1024)}])
        self.assertTrue(oracle.fingerprint(large).startswith("speech-v1:sha256:"))


class RelationsAreIndependentlyDerived(unittest.TestCase):
    def test_only_identity_and_order_changes_keep_the_fingerprint(self):
        built = oracle.build()
        same = {entry["right"] for entry in built["relations"] if entry["same"]}
        self.assertEqual(same, {"moved_and_renamed", "pronunciation_reordered"})

    def test_every_effective_change_has_a_distinct_fingerprint(self):
        built = oracle.build()
        prints = [entry["expected"]["fingerprint"] for entry in built["vectors"]]
        # Two vectors are deliberate non-changes of the request and share the base print.
        self.assertEqual(len(prints) - 2, len(set(prints)))

    def test_rejections_carry_stable_codes(self):
        codes = {entry["id"]: entry["error"] for entry in oracle.build()["rejected"]}
        self.assertEqual(codes, {
            "google_voice_unpinned": "model_not_pinned",
            "espeak_floating_model": "model_not_pinned",
            "empty_text": "empty_field:text",
            "empty_voice": "empty_field:voice_id",
            "duplicate_surface": "duplicate_pronunciation_surface",
        })


class CommittedVectorsMatchTheOracle(unittest.TestCase):
    def test_committed_goldens_are_current(self):
        self.assertEqual(oracle.GOLDENS.read_text(encoding="utf-8"),
                         oracle.render(oracle.build()))


if __name__ == "__main__":
    unittest.main()
