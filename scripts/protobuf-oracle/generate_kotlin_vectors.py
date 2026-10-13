"""Prepare Kotlin oracle inputs from reviewed expected values, never expectations from Rust."""
import json
import sys
from pathlib import Path
corpus=json.loads(Path(sys.argv[1]).read_text())
def quoted(value):
    if value is None: return 'null'
    return json.dumps(value, ensure_ascii=False).replace('$','\\$')
def initializer(message,value):
    fields={'ValidateScriptRequest':['script_json'],'SaveRevisionRequest':['expected_revision','operation_id','script_json'],'RevisionResponse':['script_id','revision','accepted_by','accepted_at','content_digest','export_digest','script_json'],'FieldIssue':['path','rule']}
    if message=='ScriptValidationResponse':
        return 'ScriptValidationResponse(listOf('+', '.join(initializer('FieldIssue',i) for i in value['issues'])+'))'
    parts=[]
    for field in fields[message]:
        v=value.get(field)
        parts.append(f'{v}uL.toLong()' if isinstance(v,int) else quoted(v))
    return message+'('+', '.join(parts)+')'
output=['fun main() {']
for case in corpus['cases']:
    output.append(f'    checkVector({quoted(case["name"])}, {case["message"]}.serializer(), {initializer(case["message"],case["value"])}, {quoted(case["protobuf_hex"])})')
output.extend([f'    println("PASS Kotlin 2.2.20 / kotlinx protobuf 1.9.0 schema oracle: {len(corpus["cases"])} vectors")','}'])
Path(sys.argv[2]).write_text('\n'.join(output)+'\n')
