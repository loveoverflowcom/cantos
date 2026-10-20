"""Generate the CMP enrollment boundary from its reviewed strict JSON schema."""
import json
import pathlib
import sys

schema_path, output_path, corpus_path, fixture_path = map(pathlib.Path, sys.argv[1:])
schema_text = schema_path.read_text()
schema = json.loads(schema_text)


def quoted(value):
    return json.dumps(value, ensure_ascii=False).replace('$', '\\$')


def title(value):
    return ''.join(part.title() for part in value.split('_'))


lines = [
    '// Generated from packages/contracts/schemas/theatre-registration-v1.schema.json. Do not edit.',
    'package com.cantos.theatre.adapters.http.registrationwire',
    'import kotlinx.serialization.Serializable',
    'import kotlinx.serialization.SerialName',
    '',
]


def emit_object(name, spec, parent=None, tag=None):
    assert spec['type'] == 'object' and spec['additionalProperties'] is False
    assert set(spec['required']) == set(spec['properties'])
    fields = {key: value for key, value in spec['properties'].items() if key != 'status' or tag is None}
    lines.append('@Serializable')
    if tag is not None:
        lines.append('@SerialName(' + quoted(tag) + ')')
    lines.append('class ' + name + '(')
    for key, value in fields.items():
        if 'enum' in value:
            field_type = name + title(key)
        else:
            assert value['type'] == 'string'
            field_type = 'String'
        lines.append('    val ' + key + ': ' + field_type + ',')
    lines.append(')' + (' : ' + parent if parent else '') + ' {')
    lines.append('    override fun toString(): String = "' + name + '([redacted])"')
    lines.append('}')
    for key, value in fields.items():
        if 'enum' in value:
            lines.extend(['@Serializable', 'enum class ' + name + title(key) + ' {'])
            for item in value['enum']:
                lines.append('    @SerialName(' + quoted(item) + ') ' + title(item) + ',')
            lines.append('}')
    lines.append('')


for definition, spec in schema['$defs'].items():
    name = title(definition)
    if 'oneOf' in spec:
        lines.extend(['@Serializable', 'sealed interface ' + name])
        for variant in spec['oneOf']:
            tag = variant['properties']['status']['const']
            emit_object(name + title(tag), variant, name, tag)
    else:
        emit_object(name, spec)

output_path.parent.mkdir(parents=True, exist_ok=True)
output_path.write_text('\n'.join(lines) + '\n')
corpus_path.parent.mkdir(parents=True, exist_ok=True)
corpus_path.write_text(
    '// Original independent contract fixtures; no expectations generated.\n'
    'package com.cantos.theatre.adapters.http\n'
    'internal const val registrationFixtureJson: String = ' + quoted(fixture_path.read_text()) + '\n'
)
