"""Independent reviewed protobuf wire-spec oracle; no Rust/Kotlin dependency.

An xtask verification primitive, not a new orchestration entrypoint. Field tables
are reviewed against the schemas; expectations never come from prost output.
"""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / "packages/contracts/fixtures/protobuf/v1"
ARTWORK_DIRECTORY = ROOT / "packages/contracts/fixtures/artwork/v1"
# Preserve the pilot table and corpus independently of the additive artwork schema.
FIELDS = {
    "ValidateScriptRequest": [(1, "script_json", "string")],
    "SaveRevisionRequest": [(1, "expected_revision", "uint64"), (2, "operation_id", "string"), (3, "script_json", "string")],
    "RevisionResponse": [(1, "script_id", "string"), (2, "revision", "uint64"), (3, "accepted_by", "string"), (4, "accepted_at", "string"), (5, "content_digest", "string"), (6, "export_digest", "string"), (7, "script_json", "string")],
    "FieldIssue": [(1, "path", "string"), (2, "rule", "string")],
    "ScriptValidationResponse": [(1, "issues", "repeated:FieldIssue")],
}
ARTWORK_FIELDS = {
    "ImageDeclarations": [(1, "provenance", "string"), (2, "rights_refs", "repeated:string")],
    "CreateImageIntentRequest": [(1, "operation_id", "string"), (2, "id", "string"), (3, "expected_sha256", "string"), (4, "expected_byte_length", "uint64"), (5, "content_type", "ImageMime"), (6, "declarations", "ImageDeclarations")],
    "ImageIntentDocument": [(1, "schema_version", "string"), (2, "id", "string"), (3, "work_id", "string"), (4, "owner_id", "string"), (5, "operation_id", "string"), (6, "request", "CreateImageIntentRequest"), (7, "created_at_ms", "uint64"), (8, "expires_at_ms", "uint64"), (9, "digest", "string"), (10, "profile_id", "string")],
    "ImageVariantMetadata": [(1, "sha256", "string"), (2, "byte_length", "uint64"), (3, "content_type", "ImageMime"), (4, "width_px", "uint32"), (5, "height_px", "uint32"), (6, "profile_id", "string")],
    "ImageAssetDocument": [(1, "schema_version", "string"), (2, "id", "string"), (3, "intent_id", "string"), (4, "work_id", "string"), (5, "owner_id", "string"), (6, "verified_media", "ImageVariantMetadata"), (7, "declarations", "ImageDeclarations"), (8, "admitted_at_ms", "uint64"), (9, "digest", "string")],
    "ImageIntentResponse": [(1, "intent", "ImageIntentDocument"), (2, "asset", "ImageAssetDocument"), (3, "state", "ImageIntentState")],
    "WorkArtworkSelection": [(1, "image_asset_id", "string"), (2, "image_asset_digest", "string"), (3, "alt_text", "string")],
    "ClearArtwork": [],
    "SetWorkArtworkRequest": [(1, "operation_id", "string"), (2, "expected_work_revision", "uint64"), (3, "artwork", "WorkArtworkSelection"), (4, "clear_artwork", "ClearArtwork")],
    "WorkArtworkResponse": [(1, "work_id", "string"), (2, "work_revision", "uint64"), (3, "artwork", "WorkArtworkSelection"), (4, "changed_at_ms", "uint64")],
}
ENUMS = {
    "ImageMime": {"image/png": 1, "image/jpeg": 2},
    "ImageIntentState": {"pending": 1, "admitted": 2, "expired": 3},
}
OPTIONAL_ARTWORK_FIELDS = {
    "ImageDeclarations": {"rights_refs"},
    "ImageIntentResponse": {"asset"},
    "SetWorkArtworkRequest": {"artwork", "clear_artwork"},
    "WorkArtworkResponse": {"artwork", "changed_at_ms"},
}


def varint(value):
    if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value < 1 << 64:
        raise ValueError("uint64 boundary")
    output = bytearray()
    while value >= 128:
        output.append((value & 127) | 128)
        value >>= 7
    output.append(value)
    return bytes(output)


def wire_value(message, value):
    """Translate the one application-only explicit null command into its wire arm."""
    if message == "SetWorkArtworkRequest" and "artwork" in value and value["artwork"] is None:
        value = dict(value)
        del value["artwork"]
        value["clear_artwork"] = {}
    return value


def encode(message, value):
    fields = FIELDS.get(message, ARTWORK_FIELDS.get(message))
    if fields is None or not isinstance(value, dict):
        raise ValueError("unknown message or non-object value")
    value = wire_value(message, value)
    if set(value) - {name for _, name, _ in fields}:
        raise ValueError("unknown field")
    result = bytearray()
    for number, name, kind in fields:
        if name not in value or value[name] is None:
            continue
        repeated = kind.startswith("repeated:")
        base_kind = kind[9:] if repeated else kind
        items = value[name] if repeated else [value[name]]
        if repeated and not isinstance(items, list):
            raise ValueError("repeated field must be a list")
        for item in items:
            if base_kind in ("uint64", "uint32") or base_kind in ENUMS:
                scalar = ENUMS[base_kind].get(item, item) if base_kind in ENUMS else item
                if base_kind == "uint32" and (not isinstance(scalar, int) or isinstance(scalar, bool) or not 0 <= scalar < 1 << 32):
                    raise ValueError("uint32 boundary")
                result.extend(varint(number << 3) + varint(scalar))
            else:
                raw = item.encode("utf-8") if base_kind == "string" else encode(base_kind, item)
                result.extend(varint(number << 3 | 2) + varint(len(raw)) + raw)
    return bytes(result)


class ArtworkWireError(ValueError):
    """Exact admission category for the independent bounded wire reader."""


def decode_artwork(message, data, *, max_bytes=32768, max_depth=4, max_fields=256, max_rights=64):
    """Read the reviewed artwork schema, with limits enforced before value allocation.

    This small independent oracle validates wire/DTO shape only. UUIDs, digest
    schemes, rights ownership and i64-backed domain bounds belong to the domain.
    """
    if len(data) > max_bytes:
        raise ArtworkWireError("TooLarge")
    field_count = 0
    missing_shape = False

    def read_varint(cursor, end):
        value = 0
        for index in range(10):
            if cursor >= end:
                raise ArtworkWireError("Truncated")
            byte = data[cursor]
            cursor += 1
            if index == 9 and byte > 1:
                raise ArtworkWireError("Malformed")
            value |= (byte & 127) << (7 * index)
            if not byte & 128:
                if index and byte == 0:
                    raise ArtworkWireError("Malformed")
                return value, cursor
        raise ArtworkWireError("Malformed")

    def read_message(kind, cursor, end, depth):
        nonlocal field_count, missing_shape
        if depth > max_depth:
            raise ArtworkWireError("TooDeep")
        fields = {number: (name, field_kind) for number, name, field_kind in ARTWORK_FIELDS[kind]}
        value = {}
        seen = set()
        rights_count = 0
        while cursor < end:
            field_count += 1
            if field_count > max_fields:
                raise ArtworkWireError("TooLarge")
            tag, cursor = read_varint(cursor, end)
            number, wire = tag >> 3, tag & 7
            if number == 0 or number > 0x1fffffff:
                raise ArtworkWireError("Malformed")
            if number not in fields:
                raise ArtworkWireError("Shape")
            name, field_kind = fields[number]
            repeated = field_kind.startswith("repeated:")
            base_kind = field_kind[9:] if repeated else field_kind
            scalar = base_kind in ("uint64", "uint32") or base_kind in ENUMS
            if wire != (0 if scalar else 2):
                raise ArtworkWireError("Shape")
            if not repeated and name in seen:
                raise ArtworkWireError("Shape")
            if kind == "SetWorkArtworkRequest" and number in (3, 4) and seen & {"artwork", "clear_artwork"}:
                raise ArtworkWireError("Shape")
            seen.add(name)
            if name == "rights_refs":
                rights_count += 1
                if rights_count > max_rights:
                    raise ArtworkWireError("TooLarge")
            if scalar:
                item, cursor = read_varint(cursor, end)
                if base_kind == "uint32" and item >= 1 << 32:
                    raise ArtworkWireError("Shape")
                if base_kind in ENUMS:
                    reverse = {number: text for text, number in ENUMS[base_kind].items()}
                    if item not in reverse:
                        raise ArtworkWireError("Shape")
                    item = reverse[item]
            else:
                length, cursor = read_varint(cursor, end)
                if length > end - cursor:
                    raise ArtworkWireError("Truncated")
                next_cursor = cursor + length
                if base_kind == "string":
                    try:
                        item = data[cursor:next_cursor].decode("utf-8")
                    except UnicodeDecodeError as error:
                        raise ArtworkWireError("InvalidUtf8") from error
                else:
                    item = read_message(base_kind, cursor, next_cursor, depth + 1)
                cursor = next_cursor
            if repeated:
                value.setdefault(name, []).append(item)
            else:
                value[name] = item
        optional = OPTIONAL_ARTWORK_FIELDS.get(kind, set())
        required = {name for _, name, _ in ARTWORK_FIELDS[kind]} - optional
        if not required <= seen:
            missing_shape = True
        if kind == "ImageDeclarations":
            value.setdefault("rights_refs", [])
        elif kind == "ImageIntentResponse":
            value.setdefault("asset", None)
        elif kind == "SetWorkArtworkRequest":
            if not seen & {"artwork", "clear_artwork"}:
                missing_shape = True
            if "clear_artwork" in value:
                del value["clear_artwork"]
                value["artwork"] = None
        elif kind == "WorkArtworkResponse":
            value.setdefault("artwork", None)
            value.setdefault("changed_at_ms", None)
        return value

    value = read_message(message, 0, len(data), 1)
    # Wire admission covers the entire envelope before required DTO presence.
    if missing_shape:
        raise ArtworkWireError("Shape")
    return value


def check_corpus(path, *, write=False):
    corpus = json.loads(path.read_text(encoding="utf-8"))
    for case in corpus["cases"]:
        expected = encode(case["message"], case.get("wire_value", case["value"])).hex()
        if write:
            case["protobuf_hex"] = expected
        elif case["protobuf_hex"] != expected:
            raise SystemExit(f"FAIL protobuf golden drift: {case['name']}")
    if write:
        path.write_text(json.dumps(corpus, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return len(corpus["cases"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write-goldens", action="store_true")
    args = parser.parse_args()
    pilot = check_corpus(DIRECTORY / "goldens.json", write=args.write_goldens)
    artwork = check_corpus(ARTWORK_DIRECTORY / "goldens.json", write=args.write_goldens)
    print(f"PASS independent protobuf goldens: {pilot} pilot + {artwork} artwork")


if __name__ == "__main__":
    main()
