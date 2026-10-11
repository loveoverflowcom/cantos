"""The independent MessagePack oracle follows the specification, not rmp-serde.

Expected bytes are written from the MessagePack format table by hand, so these tests pin the
oracle that pins the Rust codec.
"""

import unittest

import reference_msgpack as msgpack


class EncoderFollowsTheSpecification(unittest.TestCase):
    def test_scalars_use_the_smallest_format_at_every_boundary(self):
        cases = [
            (None, "c0"), (False, "c2"), (True, "c3"),
            (0, "00"), (127, "7f"), (128, "cc80"), (255, "ccff"), (256, "cd0100"),
            (65535, "cdffff"), (65536, "ce00010000"), (2**32 - 1, "ceffffffff"),
            (2**32, "cf0000000100000000"), (2**63 - 1, "cf7fffffffffffffff"),
            (-1, "ff"), (-32, "e0"), (-33, "d0df"), (-128, "d080"), (-129, "d1ff7f"),
            (-(2**63), "d38000000000000000"),
        ]
        for value, expected in cases:
            with self.subTest(value=value):
                self.assertEqual(msgpack.encode(value).hex(), expected)

    def test_string_headers_switch_at_31_255_and_65535_utf8_bytes(self):
        for length, header in [(0, "a0"), (31, "bf"), (32, "d920"), (255, "d9ff"),
                               (256, "da0100"), (65535, "daffff"), (65536, "db00010000")]:
            with self.subTest(length=length):
                encoded = msgpack.encode("x" * length)
                self.assertEqual(encoded[: len(header) // 2].hex(), header)
                self.assertEqual(len(encoded), len(header) // 2 + length)

    def test_strings_are_raw_utf8_without_normalization_or_escaping(self):
        text = "Ngày \"mai\" \\ \U0001F3AD\n"
        encoded = msgpack.encode(text)
        self.assertEqual(encoded[1:], text.encode("utf-8"))
        self.assertEqual(msgpack.decode(encoded), text)
        self.assertNotEqual(msgpack.decode(encoded), "Ngày \"mai\" \\ \U0001F3AD\n")

    def test_maps_are_named_and_keep_fixture_key_order(self):
        encoded = msgpack.encode({"b": 1, "a": None})
        self.assertEqual(encoded.hex(), "82a16201a161c0")
        self.assertEqual(list(msgpack.decode(encoded)), ["b", "a"])

    def test_container_headers_switch_at_15_and_65535_entries(self):
        self.assertEqual(msgpack.encode([0] * 15)[:1].hex(), "9f")
        self.assertEqual(msgpack.encode([0] * 16)[:3].hex(), "dc0010")
        self.assertEqual(msgpack.encode([0] * 65536)[:5].hex(), "dd00010000")
        self.assertEqual(msgpack.encode({str(i): 0 for i in range(16)})[:3].hex(), "de0010")

    def test_unsupported_values_are_refused(self):
        for value in (1.5, b"bytes", 2**64, -(2**63) - 1):
            with self.subTest(value=value), self.assertRaises(msgpack.MsgpackError):
                msgpack.encode(value)


class DecoderIsStrictAndBounded(unittest.TestCase):
    def test_exactly_one_value_is_admitted(self):
        encoded = msgpack.encode({"title": "t"})
        for data, reason in [(encoded[:-1], "truncated"), (b"", "truncated"),
                             (encoded + b"\xc0", "trailing bytes"), (b"\xc1", "0xc1")]:
            with self.subTest(reason=reason), self.assertRaisesRegex(msgpack.MsgpackError, reason):
                msgpack.decode(data)

    def test_maps_with_duplicate_or_non_string_keys_are_refused(self):
        with self.assertRaisesRegex(msgpack.MsgpackError, "duplicate map key"):
            msgpack.decode(bytes.fromhex("82a16101a16102"))
        with self.assertRaisesRegex(msgpack.MsgpackError, "not a string"):
            msgpack.decode(bytes.fromhex("810101"))

    def test_invalid_utf8_and_float_binary_extension_markers_are_refused(self):
        with self.assertRaisesRegex(msgpack.MsgpackError, "UTF-8"):
            msgpack.decode(bytes.fromhex("a2c328"))
        for marker in ("ca00000000", "c40100", "d40100"):
            with self.subTest(marker=marker), self.assertRaisesRegex(msgpack.MsgpackError, "unsupported"):
                msgpack.decode(bytes.fromhex(marker))

    def test_nesting_is_bounded(self):
        deepest = bytes([0x91] * (msgpack.MAX_DEPTH - 1)) + b"\xc0"
        self.assertIsNotNone(msgpack.decode(deepest))
        with self.assertRaisesRegex(msgpack.MsgpackError, "depth"):
            msgpack.decode(bytes([0x91] * msgpack.MAX_DEPTH) + b"\xc0")

    def test_huge_declared_lengths_fail_without_reading_past_the_input(self):
        for data in (bytes.fromhex("dbffffffff41"), bytes.fromhex("ddffffffff"),
                     bytes.fromhex("dfffffffff")):
            with self.subTest(data=data.hex()), self.assertRaisesRegex(msgpack.MsgpackError, "truncated"):
                msgpack.decode(data)


class CommittedGoldens(unittest.TestCase):
    def test_goldens_agree_with_fixtures_in_both_directions(self):
        self.assertEqual(msgpack.check_goldens(), [])

    def test_every_golden_is_a_named_map(self):
        for definition, fixture in msgpack.CASES:
            with self.subTest(definition=definition):
                first = msgpack.encode(msgpack.load_fixture(fixture))[0]
                self.assertTrue(0x80 <= first <= 0x8F or first in (0xDE, 0xDF))


if __name__ == "__main__":
    unittest.main()
