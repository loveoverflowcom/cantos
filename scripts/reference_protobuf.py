"""Independent reviewed protobuf wire-spec oracle; no Rust/Kotlin dependency.

An xtask verification primitive, not a new orchestration entrypoint. Fixtures
pin the actual pilot DTOs; this deliberately simple encoder never reads prost.
"""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / "packages/contracts/fixtures/protobuf/v1"
FIELDS = {
    "ValidateScriptRequest": [(1, "script_json", "string")],
    "SaveRevisionRequest": [(1, "expected_revision", "uint64"), (2, "operation_id", "string"), (3, "script_json", "string")],
    "RevisionResponse": [(1, "script_id", "string"), (2, "revision", "uint64"), (3, "accepted_by", "string"), (4, "accepted_at", "string"), (5, "content_digest", "string"), (6, "export_digest", "string"), (7, "script_json", "string")],
    "FieldIssue": [(1, "path", "string"), (2, "rule", "string")],
    "ScriptValidationResponse": [(1, "issues", "repeated:FieldIssue")],
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

def encode(message, value):
    if set(value) - {name for _, name, _ in FIELDS[message]}:
        raise ValueError("unknown field")
    result = bytearray()
    for number, name, kind in FIELDS[message]:
        if name not in value or value[name] is None:
            continue
        items = value[name] if kind.startswith("repeated:") else [value[name]]
        for item in items:
            if kind == "uint64":
                result.extend(varint(number << 3) + varint(item))
            else:
                raw = encode(kind[9:], item) if kind.startswith("repeated:") else item.encode("utf-8")
                result.extend(varint(number << 3 | 2) + varint(len(raw)) + raw)
    return bytes(result)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write-goldens", action="store_true")
    args = parser.parse_args()
    path = DIRECTORY / "goldens.json"
    corpus = json.loads(path.read_text())
    for case in corpus["cases"]:
        expected = encode(case["message"], case["value"]).hex()
        if args.write_goldens:
            case["protobuf_hex"] = expected
        elif case["protobuf_hex"] != expected:
            raise SystemExit(f"FAIL protobuf golden drift: {case['name']}")
    if args.write_goldens:
        path.write_text(json.dumps(corpus, ensure_ascii=False, indent=2) + "\n")
    print(f"PASS independent protobuf goldens: {len(corpus['cases'])}")

if __name__ == "__main__":
    main()
