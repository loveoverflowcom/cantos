"""Gradle primitive: embed reviewed expected values/bytes, never generate expectations."""
import json
import sys
from pathlib import Path


def quoted(value):
    return json.dumps(value, ensure_ascii=False).replace("$", "\\$")


def initializer(message, value):
    fields = {
        "ValidateScriptRequest": ["script_json"],
        "SaveRevisionRequest": ["expected_revision", "operation_id", "script_json"],
        "RevisionResponse": ["script_id", "revision", "accepted_by", "accepted_at", "content_digest", "export_digest", "script_json"],
        "FieldIssue": ["path", "rule"],
    }
    if message == "ScriptValidationResponse":
        return "ScriptValidationResponse(listOf(" + ", ".join(
            initializer("FieldIssue", issue) for issue in value["issues"]
        ) + "))"
    arguments = [
        str(value[field]) + "uL" if isinstance(value[field], int) else quoted(value[field])
        for field in fields[message]
    ]
    return message + "(" + ", ".join(arguments) + ")"


def main():
    golden_path, hostile_path, output_path = map(Path, sys.argv[1:])
    goldens = json.loads(golden_path.read_text(encoding="utf-8"))
    hostile = json.loads(hostile_path.read_text(encoding="utf-8"))
    output = [
        "// Embedded from reviewed independent wire-spec fixtures. Do not edit.",
        "package com.cantos.theatre.studiopilotfixtures",
        "internal data class PilotGolden(val name: String, val kind: StudioPilotKind, val expected: StudioPilotValue?, val hex: String)",
        "internal data class PilotHostile(val name: String, val kind: StudioPilotKind, val hex: String, val error: ProtobufFailure?)",
        "internal object CanonicalProtobufVectors {",
        "    val goldens = listOf(",
    ]
    for case in goldens["cases"]:
        expected = initializer(case["message"], case["value"]) if case["application_valid"] else "null"
        output.append(f'        PilotGolden({quoted(case["name"])}, StudioPilotKind.{case["message"]}, {expected}, {quoted(case["protobuf_hex"])}),')
    output += ["    )", "    val hostile = listOf("]
    for case in hostile["cases"]:
        error = "null" if case["error"] is None else "ProtobufFailure." + case["error"]
        output.append(f'        PilotHostile({quoted(case["name"])}, StudioPilotKind.{case["message"]}, {quoted(case["protobuf_hex"])}, {error}),')
    output += ["    )", "}"]
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(output) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
