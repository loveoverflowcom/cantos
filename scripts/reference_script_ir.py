#!/usr/bin/env python3
"""Independent c1 oracle for the already-normalized accepted fixture corpus.

This is deliberately not a validator or a runtime dependency. It uses Python
json/hashlib instead of the Rust encoder. --write-goldens is an explicit,
reviewable fixture-authoring action; tests never rewrite expected files.
"""

import argparse
import copy
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "packages/contracts/fixtures/script-ir/0.1.0/accept"


def canonical(value):
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def expected(script):
    """Project fields independently of Rust; inputs are authored in NFC."""
    content = copy.deepcopy(script)
    del content["schema_version"]
    del content["provenance"]
    del content["work"]["rights_record_id"]
    del content["work"]["source_ref"]
    del content["adaptation"]["rights_record_id"]
    del content["adaptation"]["provenance_refs"]
    content["characters"].sort(key=lambda character: character["id"])
    spoken = {}
    for act in content["episode"]["acts"]:
        for scene in act["scenes"]:
            if not scene.get("sound_cues"):
                scene.pop("sound_cues", None)
            for cue in scene.get("sound_cues", []):
                if "asset" in cue:
                    del cue["asset"]["rights_record_id"]
            for dialogue in scene["dialogues"]:
                if not dialogue.get("pronunciation_overrides"):
                    dialogue.pop("pronunciation_overrides", None)
                else:
                    dialogue["pronunciation_overrides"].sort(
                        key=lambda item: item["surface"]
                    )
                speech = {
                    key: dialogue[key]
                    for key in ("text", "delivery", "pronunciation_overrides")
                    if key in dialogue
                }
                speech["language"] = script["adaptation"]["language"]
                spoken[dialogue["id"]] = canonical(speech).decode("utf-8")
    encoded = canonical(content)
    digest = hashlib.sha256(b"cantos/script-content/c1\n" + encoded).hexdigest()
    return {
        "canonical_content": encoded.decode("utf-8"),
        "content_digest": "sir-c1:sha256:" + digest,
        "spoken_content": spoken,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write-goldens", action="store_true")
    args = parser.parse_args()
    for fixture in sorted(FIXTURES.glob("*.json")):
        if fixture.name.endswith(".expected.json"):
            continue
        oracle = expected(json.loads(fixture.read_text(encoding="utf-8")))
        target = fixture.with_suffix(".expected.json")
        if args.write_goldens:
            target.write_text(
                json.dumps(oracle, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )
        elif json.loads(target.read_text(encoding="utf-8")) != oracle:
            raise SystemExit(f"Golden mismatch: {fixture.relative_to(ROOT)}")
        print(f"c1 reference agrees: {fixture.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
