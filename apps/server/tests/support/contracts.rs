use jsonschema::{Resource, Validator};
use serde_json::{Value, json};

/// Resolve committed schemas in memory. Disabled default features prevent a
/// fixture or schema edit from silently adding network access to these tests.
pub fn validator(definition: &str) -> Validator {
    let mut api: Value = serde_json::from_str(include_str!(
        "../../../../contracts/http/script-api.schema.json"
    ))
    .expect("valid committed HTTP schema JSON");
    let script: Value =
        serde_json::from_str(include_str!("../../../../contracts/script-ir/schema.json"))
            .expect("valid committed Script IR schema JSON");
    let script_id = script["$id"]
        .as_str()
        .expect("Script IR schema has a stable identifier")
        .to_owned();
    assert!(api["$defs"].get(definition).is_some());
    api.as_object_mut()
        .expect("HTTP schema object")
        .remove("anyOf");
    api["$ref"] = json!(format!("#/$defs/{definition}"));

    jsonschema::options()
        .should_validate_formats(true)
        .with_resource(
            script_id,
            Resource::from_contents(script).expect("valid Script IR schema resource"),
        )
        .build(&api)
        .expect("valid committed HTTP definition with local Script IR reference")
}
