"""Existing Gradle fixture harness primitive; no native artwork client is generated."""
import importlib.util
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
ORACLE = ROOT / 'scripts/protobuf-oracle'
spec = importlib.util.spec_from_file_location('artwork_kotlin_vectors', ORACLE/'generate_kotlin_vectors.py')
vectors = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vectors)


def kotlin_string(value):
    # JVM class-file string constants have a 65535-byte modified UTF-8 ceiling.
    # Construct large hostile hex at runtime instead of constant-folding it.
    if len(value) <= 8192:
        return vectors.quoted(value)
    chunks = ', '.join(vectors.quoted(value[index:index+8192]) for index in range(0, len(value), 8192))
    return 'listOf(' + chunks + ').joinToString("")'


def main():
    golden_path, hostile_path, output_path = map(Path, sys.argv[1:])
    goldens = json.loads(golden_path.read_text(encoding='utf-8'))
    hostile = json.loads(hostile_path.read_text(encoding='utf-8'))
    assert goldens['contract'] == hostile['contract'] == 'cantos.studio.artwork.v1'
    # The isolated schema oracle remains the fixture mirror; include it only in
    # commonTest, in its own package, never in shipping commonMain/native routes.
    mirror = (ORACLE/'Oracle.kt').read_text(encoding='utf-8')
    marker = 'import kotlinx.serialization.Serializable'
    mirror = mirror.replace(marker, 'package com.cantos.theatre.artworkfixtures\n\n'+marker, 1)
    output = [
        '// Embedded from reviewed independent wire-spec fixtures. Do not edit.', mirror,
        'internal data class ArtworkGolden(val name: String, val message: String, val applicationValid: Boolean, val hex: String, val verifySerializer: () -> Unit)',
        'internal data class ArtworkHostile(val name: String, val message: String, val hex: String, val error: String?)',
        'internal object CanonicalArtworkVectors {',
        '    val goldens = listOf(',
    ]
    for case in goldens['cases']:
        line = vectors.vector_line(case).strip()
        output.append(f'        ArtworkGolden({vectors.quoted(case["name"])}, {vectors.quoted(case["message"])}, {str(case["application_valid"]).lower()}, {kotlin_string(case["protobuf_hex"])}) {{ {line} }},')
    output += ['    )', '    val hostile = listOf(']
    for case in hostile['cases']:
        output.append(f'        ArtworkHostile({vectors.quoted(case["name"])}, {vectors.quoted(case["message"])}, {kotlin_string(case["protobuf_hex"])}, {vectors.quoted(case["error"])}),')
    output += ['    )', '}']
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text('\n'.join(output) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
