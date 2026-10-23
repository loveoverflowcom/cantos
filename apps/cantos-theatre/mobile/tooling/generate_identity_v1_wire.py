"""Generate strict flat Identity v1 response DTOs from the owning public schema."""
import json
import pathlib
import sys

schema_path, output_path = map(pathlib.Path, sys.argv[1:])
schema = json.loads(schema_path.read_text())


def quoted(value):
    return json.dumps(value).replace('$', '\\$')


lines = [
    '// Generated from services/cantos-server/contracts/auth/identity-auth/v1/identity-auth.schema.json. Do not edit.',
    'package com.cantos.theatre.adapters.http.identityv1wire',
    '',
    'import kotlinx.serialization.Serializable',
    'import kotlinx.serialization.json.JsonObject',
    'import kotlinx.serialization.json.JsonPrimitive',
    'import kotlinx.serialization.json.booleanOrNull',
    'import kotlinx.serialization.json.longOrNull',
    '',
]
for spec in schema['oneOf']:
    assert spec['type'] == 'object' and spec['additionalProperties'] is False
    assert set(spec['required']) == set(spec['properties'])
    name = spec['title']
    lines.extend(['@Serializable', 'internal class ' + name + '('])
    predicates = []
    for key, field in spec['properties'].items():
        kind = field.get('type')
        if 'const' in field:
            kind = 'boolean' if isinstance(field['const'], bool) else 'string'
        if 'enum' in field:
            kind = 'string'
        assert kind in ('boolean', 'string', 'integer')
        field_type = {'boolean': 'Boolean', 'string': 'String', 'integer': 'Long'}[kind]
        lines.append('    val ' + key + ': ' + field_type + ',')
        primitive = '(value[' + quoted(key) + '] as? JsonPrimitive)'
        conversion = {'boolean': 'booleanOrNull', 'integer': 'longOrNull', 'string': 'content'}[kind]
        condition = 'it.isString' if kind == 'string' else '!it.isString'
        expression = primitive + '?.takeIf { ' + condition + ' }?.' + conversion
        predicates.append(expression + ' != null')
        if 'const' in field:
            predicates.append(expression + ' == ' + quoted(field['const']))
        if 'minimum' in field:
            predicates.append('(' + expression + ' ?: 0) >= ' + str(field['minimum']))
        if 'pattern' in field:
            predicates.append(expression + '?.matches(Regex(' + quoted(field['pattern']) + ')) == true')
        if 'enum' in field:
            predicates.append(expression + ' in setOf(' + ', '.join(map(quoted, field['enum'])) + ')')
    lines.extend([
        ') {',
        '    override fun toString(): String = "' + name + '([redacted])"',
        '    companion object {',
        '        fun accepts(value: JsonObject): Boolean =',
        '            value.keys == setOf(' + ', '.join(map(quoted, spec['properties'])) + ') &&',
        '            ' + ' &&\n            '.join(predicates),
        '    }',
        '}',
        '',
    ])
output_path.parent.mkdir(parents=True, exist_ok=True)
output_path.write_text('\n'.join(lines))
