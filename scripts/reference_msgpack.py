#!/usr/bin/env python3
"""Independent MessagePack oracle for the Cantos wire goldens.

The encoder writes a JSON value as MessagePack using the smallest format for each nil, boolean,
integer, string, array and map, keeping object keys in their fixture order. The decoder is
strict: exactly one value, no trailing bytes, string map keys only, no duplicate keys, bounded
nesting and no float, binary or extension types (no Cantos DTO uses them).

It shares no code with rmp-serde. Rust agreeing with these bytes is evidence about the wire
format itself, not about one library round-tripping its own output.

    python3 scripts/reference_msgpack.py                  # check the committed goldens
    python3 scripts/reference_msgpack.py --write-goldens  # deliberate regeneration only
"""
import argparse
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "packages" / "contracts" / "fixtures" / "studio" / "v1"
GOLDENS = FIXTURES / "msgpack-goldens.json"
# Matches cantos_contracts::wire::MAX_MSGPACK_DEPTH: a value nests fewer arrays/maps than this.
MAX_DEPTH = 16
# Contract DTO definition -> JSON fixture. Route support is listed in studio-v1.md; these pin
# the representation of each shape, including null, nested arrays and Unicode text.
CASES = (
    ("Revision", "revision.json"),
    ("SaveRevision", "save-revision.json"),
    ("ValidateScript", "validate-script.json"),
    ("ScriptValidation", "validation.json"),
    ("History", "history.json"),
    ("Review", "review.json"),
)


class MsgpackError(ValueError):
    """Bytes that are not exactly one supported MessagePack value."""


def _length_header(length, fixed_base, fixed_limit, markers):
    if length < fixed_limit:
        return bytes([fixed_base | length])
    for marker, fmt, limit in markers:
        if length <= limit:
            return bytes([marker]) + struct.pack(fmt, length)
    raise MsgpackError("length exceeds MessagePack limits")


def _encode_int(value):
    if 0 <= value <= 0x7F:
        return bytes([value])
    if -32 <= value < 0:
        return struct.pack(">b", value)
    if value > 0:
        for marker, fmt, limit in ((0xCC, ">B", 0xFF), (0xCD, ">H", 0xFFFF),
                                   (0xCE, ">I", 0xFFFFFFFF), (0xCF, ">Q", 0xFFFFFFFFFFFFFFFF)):
            if value <= limit:
                return bytes([marker]) + struct.pack(fmt, value)
    else:
        for marker, fmt, limit in ((0xD0, ">b", 0x80), (0xD1, ">h", 0x8000),
                                   (0xD2, ">i", 0x80000000), (0xD3, ">q", 0x8000000000000000)):
            if -value <= limit:
                return bytes([marker]) + struct.pack(fmt, value)
    raise MsgpackError("integer outside the 64-bit range")


def encode(value):
    """Smallest-format MessagePack for a JSON value; dict keys keep their insertion order."""
    if value is None:
        return b"\xc0"
    if value is False:
        return b"\xc2"
    if value is True:
        return b"\xc3"
    if isinstance(value, int):
        return _encode_int(value)
    if isinstance(value, str):
        data = value.encode("utf-8")
        header = _length_header(len(data), 0xA0, 32,
                                ((0xD9, ">B", 0xFF), (0xDA, ">H", 0xFFFF), (0xDB, ">I", 0xFFFFFFFF)))
        return header + data
    if isinstance(value, list):
        header = _length_header(len(value), 0x90, 16, ((0xDC, ">H", 0xFFFF), (0xDD, ">I", 0xFFFFFFFF)))
        return header + b"".join(encode(item) for item in value)
    if isinstance(value, dict):
        header = _length_header(len(value), 0x80, 16, ((0xDE, ">H", 0xFFFF), (0xDF, ">I", 0xFFFFFFFF)))
        return header + b"".join(encode(key) + encode(item) for key, item in value.items())
    raise MsgpackError(f"unsupported value type: {type(value).__name__}")


class _Reader:
    def __init__(self, data):
        self.data = data
        self.offset = 0

    def take(self, count):
        end = self.offset + count
        if end > len(self.data):
            raise MsgpackError("truncated")
        chunk = self.data[self.offset:end]
        self.offset = end
        return chunk

    def unpack(self, fmt):
        return struct.unpack(fmt, self.take(struct.calcsize(fmt)))[0]


_UINTS = {0xCC: ">B", 0xCD: ">H", 0xCE: ">I", 0xCF: ">Q"}
_SINTS = {0xD0: ">b", 0xD1: ">h", 0xD2: ">i", 0xD3: ">q"}
_STRS = {0xD9: ">B", 0xDA: ">H", 0xDB: ">I"}
_ARRAYS = {0xDC: ">H", 0xDD: ">I"}
_MAPS = {0xDE: ">H", 0xDF: ">I"}


def _read(reader, depth):
    marker = reader.unpack(">B")
    if marker <= 0x7F:
        return marker
    if marker >= 0xE0:
        return marker - 0x100
    if marker == 0xC0:
        return None
    if marker == 0xC2:
        return False
    if marker == 0xC3:
        return True
    if marker in _UINTS:
        return reader.unpack(_UINTS[marker])
    if marker in _SINTS:
        return reader.unpack(_SINTS[marker])
    if 0xA0 <= marker <= 0xBF or marker in _STRS:
        length = marker & 0x1F if marker <= 0xBF else reader.unpack(_STRS[marker])
        try:
            return reader.take(length).decode("utf-8")
        except UnicodeDecodeError:
            raise MsgpackError("invalid UTF-8 string") from None
    if 0x90 <= marker <= 0x9F or marker in _ARRAYS or 0x80 <= marker <= 0x8F or marker in _MAPS:
        if depth + 1 >= MAX_DEPTH:
            raise MsgpackError("depth limit exceeded")
        if 0x90 <= marker <= 0x9F or marker in _ARRAYS:
            length = marker & 0x0F if marker <= 0x9F else reader.unpack(_ARRAYS[marker])
            return [_read(reader, depth + 1) for _ in range(length)]
        length = marker & 0x0F if marker <= 0x8F else reader.unpack(_MAPS[marker])
        result = {}
        for _ in range(length):
            key = _read(reader, depth + 1)
            if not isinstance(key, str):
                raise MsgpackError("map key is not a string")
            if key in result:
                raise MsgpackError(f"duplicate map key: {key}")
            result[key] = _read(reader, depth + 1)
        return result
    raise MsgpackError(f"unsupported marker 0x{marker:02x}")


def decode(data):
    """Exactly one supported MessagePack value; trailing bytes are an error."""
    reader = _Reader(bytes(data))
    value = _read(reader, 0)
    if reader.offset != len(reader.data):
        raise MsgpackError("trailing bytes")
    return value


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_fixture(name):
    return json.loads((FIXTURES / name).read_text(encoding="utf-8"), object_pairs_hook=_unique_object)


def golden_document():
    return {
        "media_type": "application/msgpack",
        "encoding": "named maps in contract field order; smallest nil, bool, integer, string, "
                    "array and map formats",
        "generator": "python3 scripts/reference_msgpack.py --write-goldens",
        "cases": [{"definition": definition, "fixture": fixture,
                   "msgpack_hex": encode(load_fixture(fixture)).hex()}
                  for definition, fixture in CASES],
    }


def check_goldens():
    """Return a list of problems; empty when goldens, fixtures and both directions agree."""
    problems = []
    committed = json.loads(GOLDENS.read_text(encoding="utf-8"), object_pairs_hook=_unique_object)
    expected = golden_document()
    if committed != expected:
        problems.append(f"{GOLDENS.relative_to(ROOT)} differs from the reference encoding; "
                        "regenerate deliberately with --write-goldens")
    for case in committed.get("cases", []):
        fixture = load_fixture(case["fixture"])
        try:
            decoded = decode(bytes.fromhex(case["msgpack_hex"]))
        except (ValueError, MsgpackError) as error:
            problems.append(f"{case['fixture']}: golden does not decode: {error}")
            continue
        if decoded != fixture or json.dumps(decoded) != json.dumps(fixture):
            problems.append(f"{case['fixture']}: golden decodes to a different value or key order")
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--write-goldens", action="store_true",
                        help="rewrite msgpack-goldens.json from the JSON fixtures")
    args = parser.parse_args()
    if args.write_goldens:
        GOLDENS.write_text(json.dumps(golden_document(), indent=2) + "\n", encoding="utf-8")
        print(f"wrote {GOLDENS.relative_to(ROOT)}")
        return 0
    problems = check_goldens()
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(f"PASS: {len(CASES)} MessagePack goldens agree with the independent encoder and decoder")
    return 0


if __name__ == "__main__":
    sys.exit(main())
