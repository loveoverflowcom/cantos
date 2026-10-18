"""Generate strict Kotlin wire types from the reviewed Library schema; never a second DTO source."""
import json
import pathlib
import sys
schema_path, fixtures_path, output_path, test_path = map(pathlib.Path, sys.argv[1:])
schema_text = schema_path.read_text()
schema = json.loads(schema_text)
def quoted(value):
    return json.dumps(value, ensure_ascii=False).replace('$', '\\$')
def kotlin_type(spec):
    if '$ref' in spec:
        return spec['$ref'].split('/')[-1]
    if 'anyOf' in spec:
        real = [item for item in spec['anyOf'] if item.get('type') != 'null']
        assert len(real) == 1 and len(spec['anyOf']) == 2
        return kotlin_type(real[0]) + '?'
    return {'string': 'String', 'integer': 'Long', 'boolean': 'Boolean', 'array': lambda: 'List<' + kotlin_type(spec['items']) + '>'}[spec['type']]() if spec['type'] == 'array' else {'string': 'String', 'integer': 'Long', 'boolean': 'Boolean'}[spec['type']]
lines = ['// Generated from packages/contracts/schema/theatre/library/v1.schema.json. Do not edit.',
         'package com.cantos.theatre.adapters.library.wire',
         'import kotlinx.serialization.Serializable', 'import kotlinx.serialization.SerialName', '']
for name, spec in schema['$defs'].items():
    if 'enum' in spec:
        lines += ['@Serializable', 'enum class ' + name + ' { ' + ', '.join(spec['enum']) + ' }', '']
    elif 'oneOf' in spec:
        lines += ['@Serializable', 'sealed interface ' + name + ' {']
        for variant in spec['oneOf']:
            tag = variant['properties']['resource']['enum'][0]
            lines += ['    @Serializable', '    @SerialName(' + quoted(tag) + ')', '    data class ' + tag.title() + '(val current: ' + kotlin_type(variant['properties']['current']) + ') : ' + name]
        lines += ['}', '']
    else:
        assert spec['type'] == 'object' and spec['additionalProperties'] is False
        assert set(spec['required']) == set(spec['properties'])
        lines += ['@Serializable', 'data class ' + name + '(']
        for key, value in spec['properties'].items():
            # Inline single-value enums are strings checked by the generated schema validator.
            lines += ['    val ' + key + ': ' + kotlin_type(value) + ',']
        lines += [')', '']
lines += ['internal const val librarySchemaJson: String = ' + quoted(schema_text)]
output_path.parent.mkdir(parents=True, exist_ok=True)
output_path.write_text('\n'.join(lines) + '\n')
fixtures = sorted(fixtures_path.glob('*.json'))
assert fixtures
text = '// Generated from the canonical Library fixture bytes. Do not edit.\npackage com.cantos.theatre.adapters.library\ninternal val libraryFixtures = mapOf(\n'
for fixture in fixtures:
    text += '    ' + quoted(fixture.name) + ' to ' + quoted(fixture.read_text()) + ',\n'
text += ')\n'
test_path.parent.mkdir(parents=True, exist_ok=True)
test_path.write_text(text)
