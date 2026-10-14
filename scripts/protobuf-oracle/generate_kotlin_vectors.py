"""Prepare Kotlin oracle inputs from reviewed values, never expectations from Rust."""
import json
import sys
from pathlib import Path

FIELDS = {
    'ValidateScriptRequest': [('script_json', 'string')],
    'SaveRevisionRequest': [('expected_revision', 'uint64'), ('operation_id', 'string'), ('script_json', 'string')],
    'RevisionResponse': [('script_id', 'string'), ('revision', 'uint64'), ('accepted_by', 'string'), ('accepted_at', 'string'), ('content_digest', 'string'), ('export_digest', 'string'), ('script_json', 'string')],
    'FieldIssue': [('path', 'string'), ('rule', 'string')],
    'ScriptValidationResponse': [('issues', 'repeated:FieldIssue')],
    'ImageDeclarations': [('provenance', 'string'), ('rights_refs', 'repeated:string')],
    'CreateImageIntentRequest': [('operation_id', 'string'), ('id', 'string'), ('expected_sha256', 'string'), ('expected_byte_length', 'uint64'), ('content_type', 'ImageMime'), ('declarations', 'ImageDeclarations')],
    'ImageIntentDocument': [('schema_version', 'string'), ('id', 'string'), ('work_id', 'string'), ('owner_id', 'string'), ('operation_id', 'string'), ('request', 'CreateImageIntentRequest'), ('created_at_ms', 'uint64'), ('expires_at_ms', 'uint64'), ('digest', 'string'), ('profile_id', 'string')],
    'ImageVariantMetadata': [('sha256', 'string'), ('byte_length', 'uint64'), ('content_type', 'ImageMime'), ('width_px', 'uint32'), ('height_px', 'uint32'), ('profile_id', 'string')],
    'ImageAssetDocument': [('schema_version', 'string'), ('id', 'string'), ('intent_id', 'string'), ('work_id', 'string'), ('owner_id', 'string'), ('verified_media', 'ImageVariantMetadata'), ('declarations', 'ImageDeclarations'), ('admitted_at_ms', 'uint64'), ('digest', 'string')],
    'ImageIntentResponse': [('intent', 'ImageIntentDocument'), ('asset', 'ImageAssetDocument'), ('state', 'ImageIntentState')],
    'WorkArtworkSelection': [('image_asset_id', 'string'), ('image_asset_digest', 'string'), ('alt_text', 'string')],
    'WorkArtworkResponse': [('work_id', 'string'), ('work_revision', 'uint64'), ('artwork', 'WorkArtworkSelection'), ('changed_at_ms', 'uint64')],
}
ENUMS = {
    'ImageMime': {'image/png': 'Png', 'image/jpeg': 'Jpeg', 0: 'Unspecified'},
    'ImageIntentState': {'pending': 'Pending', 'admitted': 'Admitted', 'expired': 'Expired', 0: 'Unspecified'},
}


def quoted(value):
    if value is None:
        return 'null'
    return json.dumps(value, ensure_ascii=False).replace('$', '\\$')


def scalar(kind, value):
    if value is None:
        return 'null'
    if kind.startswith('repeated:'):
        return 'listOf(' + ', '.join(scalar(kind[9:], item) for item in value) + ')'
    if kind in ('uint64', 'uint32'):
        return f'{value}uL.toLong()'
    if kind == 'string':
        return quoted(value)
    if kind in ENUMS:
        return kind + '.' + ENUMS[kind][value]
    return initializer(kind, value)


def initializer(message, value):
    if message == 'ClearArtwork':
        return 'ClearArtwork'
    if message == 'SetWorkArtworkRequest':
        change = 'null'
        if 'clear_artwork' in value or ('artwork' in value and value['artwork'] is None):
            change = 'ArtworkChange.Clear(ClearArtwork)'
        elif 'artwork' in value:
            change = 'ArtworkChange.Set(' + initializer('WorkArtworkSelection', value['artwork']) + ')'
        return 'SetWorkArtworkRequest(' + scalar('string', value.get('operation_id')) + ', ' + scalar('uint64', value.get('expected_work_revision')) + ', ' + change + ')'
    parts = []
    for field, kind in FIELDS[message]:
        default = [] if kind.startswith('repeated:') else None
        parts.append(scalar(kind, value.get(field, default)))
    return message + '(' + ', '.join(parts) + ')'


def vector_line(case):
    return f'    checkVector({quoted(case["name"])}, {case["message"]}.serializer(), {initializer(case["message"], case.get("wire_value", case["value"]))}, {quoted(case["protobuf_hex"])})'


def main():
    # Retain the original two-argument pilot interface; artwork is an optional third input.
    paths = [Path(sys.argv[1])] + [Path(path) for path in sys.argv[3:]]
    cases = [case for path in paths for case in json.loads(path.read_text(encoding='utf-8'))['cases']]
    output = ['fun main() {'] + [vector_line(case) for case in cases]
    output.extend([f'    println("PASS Kotlin 2.2.20 / kotlinx protobuf 1.9.0 schema oracle: {len(cases)} vectors")', '}'])
    Path(sys.argv[2]).write_text('\n'.join(output) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
