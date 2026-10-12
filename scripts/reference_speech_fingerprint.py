#!/usr/bin/env python3
"""Independent oracle for the Cantos speech fingerprint (`speech-v1`).

It rebuilds the canonical effective-input document and its SHA-256 identity from the field
rules in packages/contracts/clip-reuse-v1.md using only the Python standard library. It shares
no code with `services/cantos-api/src/speech_fingerprint.rs`: Rust agreeing with
`packages/contracts/fixtures/clip-reuse/v1/speech-vectors.json` is evidence about the contract,
not about one implementation reproducing itself.

The vectors below are the single hand-written source. The oracle derives every expected
document, digest, rejection and same/different relation from its own rules.

    python3 scripts/reference_speech_fingerprint.py                  # check the committed vectors
    python3 scripts/reference_speech_fingerprint.py --write-goldens  # deliberate regeneration only
"""
import argparse
import copy
import hashlib
import json
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GOLDENS = (ROOT / "packages" / "contracts" / "fixtures" / "clip-reuse" / "v1"
           / "speech-vectors.json")
SCHEME = "speech-v1"
DOMAIN_TAG = b"cantos/speech-fingerprint/v1\n"
# Only exactly pinned provider models may be reused across runs (contract, "Model pinning").
PINNED_MODELS = {("espeak-ng-local", "espeak-ng-1.52.0")}
# Matches speech_fingerprint::MAX_CANONICAL_BYTES: the registry stores the canonical document and
# admission bounds a line and its settings below this, so a fingerprinted request is registrable.
MAX_CANONICAL_BYTES = 2 * 1024 * 1024
REQUIRED_NONEMPTY = ("text", "language", "provider_id", "model_id", "voice_id",
                     "adapter_version", "output_contract")


class Rejected(ValueError):
    """An input the contract gives no fingerprint; the message is the stable error code."""


def canonical_document(resolved):
    """The effective synthesis request. Dialogue, character and rights identity are not in it."""
    for field in REQUIRED_NONEMPTY:
        if resolved[field] == "":
            raise Rejected(f"empty_field:{field}")
    if (resolved["provider_id"], resolved["model_id"]) not in PINNED_MODELS:
        raise Rejected("model_not_pinned")
    surfaces = [entry["surface"] for entry in resolved["pronunciation"]]
    if len(set(surfaces)) != len(surfaces):
        raise Rejected("duplicate_pronunciation_surface")
    performance = resolved["performance"]
    return {
        "adapter_version": resolved["adapter_version"],
        "language": resolved["language"],
        "model_id": resolved["model_id"],
        "output_contract": resolved["output_contract"],
        "performance": {
            "emotion": performance["emotion"],
            "intensity_permille": performance["intensity_permille"],
            "pitch_semitones": performance["pitch_semitones"],
            "rate_permille": performance["rate_permille"],
        },
        "pronunciation": sorted(
            ({"replacement": entry["replacement"], "surface": entry["surface"]}
             for entry in resolved["pronunciation"]),
            key=lambda entry: entry["surface"]),
        "provider_id": resolved["provider_id"],
        "text": resolved["text"],
        "voice_id": resolved["voice_id"],
    }


def canonical_json(resolved):
    document = json.dumps(canonical_document(resolved), sort_keys=True, ensure_ascii=False,
                          separators=(",", ":"))
    if len(document.encode("utf-8")) > MAX_CANONICAL_BYTES:
        raise Rejected("input_too_large")
    return document


def fingerprint(resolved):
    digest = hashlib.sha256(DOMAIN_TAG + canonical_json(resolved).encode("utf-8"))
    return f"{SCHEME}:sha256:{digest.hexdigest()}"


def base_input():
    return {
        "dialogue_id": "dialogue-02",
        "character_id": "an",
        "text": "Ngày mai, mình có diễn tiếp không?",
        "language": "vi-VN",
        "provider_id": "espeak-ng-local",
        "model_id": "espeak-ng-1.52.0",
        "voice_id": "vi-vn-x-south",
        "voice_rights_record_id": "voice-espeak",
        "adapter_version": "espeak-ng-local-v1",
        "output_contract": "wav-pcm-s16le-mono-22050-v1",
        "performance": {"rate_permille": 1000, "pitch_semitones": 0,
                        "emotion": "neutral", "intensity_permille": 0},
        "pronunciation": [{"surface": "Vọng Đài", "replacement": "Vọng Đài"},
                          {"surface": "An", "replacement": "Ân"}],
    }


def variant(**changes):
    resolved = base_input()
    for path, value in changes.items():
        target = resolved
        *parents, leaf = path.split("__")
        for parent in parents:
            target = target[parent]
        target[leaf] = value
    return resolved


def vectors():
    """(id, note, input). `base` anchors every same/different relation."""
    reordered = base_input()
    reordered["pronunciation"].reverse()
    return [
        ("base", "An, dialogue-02: the position-free reference line", base_input()),
        ("moved_and_renamed",
         "Another dialogue, character and rights record; the synthesis request is identical",
         variant(dialogue_id="dialogue-77", character_id="narrator",
                 voice_rights_record_id="voice-other-record")),
        ("pronunciation_reordered", "The same pronunciation set in another order", reordered),
        ("text_changed", "One character of effective text changed",
         variant(text="Ngày mai, mình có diễn tiếp không.")),
        ("text_nfd", "Unicode-equivalent but byte-different text is a different request",
         variant(text=unicodedata.normalize("NFD", base_input()["text"]))),
        ("voice_changed", "Another provider voice", variant(voice_id="vi")),
        ("language_changed", "Another language tag", variant(language="vi")),
        ("pronunciation_replacement_changed", "One replacement respelled", variant(
            pronunciation=[{"surface": "Vọng Đài", "replacement": "Vọng Đài"},
                           {"surface": "An", "replacement": "Anh"}])),
        ("pronunciation_removed", "A pronunciation rule removed",
         variant(pronunciation=[{"surface": "An", "replacement": "Ân"}])),
        ("rate_changed", "Another speaking rate", variant(performance__rate_permille=900)),
        ("pitch_changed", "Another pitch", variant(performance__pitch_semitones=1)),
        ("emotion_inherited", "Emotion left to the script is not 'neutral'",
         variant(performance__emotion=None)),
        ("intensity_changed", "Another intensity", variant(performance__intensity_permille=10)),
        ("adapter_changed", "Another adapter version",
         variant(adapter_version="espeak-ng-local-v2")),
        ("output_contract_changed", "Another output contract",
         variant(output_contract="wav-pcm-s16le-mono-24000-v1")),
        ("escaped_text", "Quotes, backslash and control characters are escaped, not dropped",
         variant(text="Anh nói: \"đi\" \\ về\tnhà\u0001")),
    ]


def rejected():
    return [
        ("google_voice_unpinned", "No revision identifies the voice, so equal names prove nothing",
         variant(provider_id="google-cloud-tts", model_id="vi-VN-Chirp3-HD-Aoede")),
        ("espeak_floating_model", "A moving model alias is not a pin",
         variant(model_id="espeak-ng-latest")),
        ("empty_text", "Nothing to say", variant(text="")),
        ("empty_voice", "No voice", variant(voice_id="")),
        ("duplicate_surface", "Two replacements for one surface are ambiguous",
         variant(pronunciation=[{"surface": "An", "replacement": "Ân"},
                                {"surface": "An", "replacement": "Anh"}])),
    ]


def build():
    accepted = []
    for identifier, note, resolved in vectors():
        accepted.append({
            "id": identifier,
            "note": note,
            "input": resolved,
            "expected": {"canonical_json": canonical_json(resolved),
                         "fingerprint": fingerprint(resolved)},
        })
    refused = []
    for identifier, note, resolved in rejected():
        try:
            canonical_document(resolved)
        except Rejected as error:
            code = str(error)
        else:
            raise SystemExit(f"{identifier}: oracle unexpectedly accepted the input")
        refused.append({"id": identifier, "note": note, "input": resolved, "error": code})
    base = next(entry for entry in accepted if entry["id"] == "base")
    relations = [
        {"left": "base", "right": entry["id"],
         "same": entry["expected"]["fingerprint"] == base["expected"]["fingerprint"]}
        for entry in accepted if entry["id"] != "base"
    ]
    return {
        "scheme": SCHEME,
        "domain_tag": DOMAIN_TAG.decode("ascii"),
        "vectors": accepted,
        "rejected": refused,
        "relations": relations,
    }


def render(document):
    return json.dumps(document, ensure_ascii=False, indent=2, sort_keys=False) + "\n"


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--write-goldens", action="store_true",
                        help="rewrite the committed vectors; review the diff before committing")
    arguments = parser.parse_args(argv)
    rendered = render(build())
    if arguments.write_goldens:
        GOLDENS.parent.mkdir(parents=True, exist_ok=True)
        GOLDENS.write_text(rendered, encoding="utf-8")
        print(f"wrote {GOLDENS.relative_to(ROOT)}")
        return 0
    if not GOLDENS.exists() or GOLDENS.read_text(encoding="utf-8") != rendered:
        print(f"{GOLDENS.relative_to(ROOT)} differs from the oracle output; "
              "regenerate deliberately with --write-goldens and review the diff",
              file=sys.stderr)
        return 1
    print(f"{GOLDENS.relative_to(ROOT)}: {len(build()['vectors'])} vectors match the oracle")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
