#!/usr/bin/env python3
"""Independent stdlib digest oracle and optional external JSON Schema oracle."""
import argparse
import copy
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema", action="store_true")
    args = parser.parse_args()
    accepted = json.loads((ROOT / "fixtures/accepted/episode.json").read_text())
    canonical = copy.deepcopy(accepted)
    canonical.pop("schema_version")
    canonical["work"].pop("source_ref")
    canonical["work"].pop("rights_ref")
    canonical["characters"].sort(key=lambda character: character["id"])
    for act in canonical["episode"]["acts"]:
        for scene in act["scenes"]:
            for cue in scene.get("sound_cues", []):
                cue.pop("rights_ref", None)
    encoded = json.dumps(canonical, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    expected = (ROOT / "fixtures/accepted/episode.canonical.json").read_bytes().removesuffix(b"\n")
    assert encoded == expected, "canonical golden differs"
    digest = "cantos-script-v1:sha256:" + hashlib.sha256(b"cantos/script-content/v1\n" + encoded).hexdigest()
    assert digest == (ROOT / "fixtures/accepted/episode.digest.txt").read_text().strip()
    print("PASS: independent canonical bytes and domain-separated SHA-256 golden")
    if args.schema:
        from jsonschema import Draft202012Validator

        schema = json.loads((ROOT / "schema.json").read_text())
        Draft202012Validator.check_schema(schema)
        validator = Draft202012Validator(schema)
        validator.validate(accepted)
        cases = json.loads((ROOT / "fixtures/rejected/expected.json").read_text())
        for case in cases:
            value = json.loads((ROOT / "fixtures/rejected" / case["file"]).read_text())
            shape_valid = validator.is_valid(value)
            assert shape_valid == (case["layer"] == "semantic"), case["file"]
        print(f"PASS: schema accepts 1 valid fixture and classifies {len(cases)} rejected fixtures")


if __name__ == "__main__":
    main()
