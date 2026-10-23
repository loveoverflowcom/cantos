//! Admission of source/binding facts and retention of those bindings in edited proposals.
//! Current ownership, registry integrity and authorization remain application/SQL checks.
use super::AdaptationRejection;
use crate::contract::studio::ImportResponse;
use serde_json::{json, Value};

use super::{array, problem, script_problem};
use crate::script_ir::{read_script, ScriptContent};

/// Raw binding facts, normally loaded by the application. Public fields are not evidence of
/// provenance or current authorization; every entry point admits them before consuming them.
#[derive(Clone, Debug)]
pub struct TrustedBinding {
    pub source_id: String,
    pub source_sha256: String,
    pub generation_record_id: String,
    pub rights_record_id: String,
    pub id_namespace: String,
    pub base_script_json: Option<String>,
}

/// Intrinsic, immutable source/binding agreement only. It cannot be constructed from raw
/// fields by the materializer, and borrowing pins the exact source and facts during use.
pub(super) struct BoundSource<'a> {
    source: &'a ImportResponse,
    facts: &'a TrustedBinding,
}

impl<'a> BoundSource<'a> {
    pub(super) fn source(&self) -> &'a ImportResponse {
        self.source
    }

    pub(super) fn facts(&self) -> &'a TrustedBinding {
        self.facts
    }
}

pub(super) fn check_binding<'a>(
    source: &'a ImportResponse,
    binding: &'a TrustedBinding,
) -> Result<BoundSource<'a>, AdaptationRejection> {
    let valid_id = |id: &str| {
        !id.is_empty()
            && id.len() <= 64
            && id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte)
            })
    };
    if source.id != binding.source_id
        || source.sha256 != binding.source_sha256
        || !valid_id(&binding.generation_record_id)
        || !valid_id(&binding.rights_record_id)
        || binding.id_namespace.len() != 32
        || !binding
            .id_namespace
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(problem("binding_mismatch", "/source_id", "trusted_source"));
    }
    Ok(BoundSource {
        source,
        facts: binding,
    })
}

/// Edited acceptance uses the existing validator and retains the complete trusted evidence set.
/// Content may change, but a creator cannot remove or replace the source/attempt/rights bindings.
pub fn admit_edited_proposal(
    json: &str,
    source: &ImportResponse,
    binding: &TrustedBinding,
) -> Result<ScriptContent, AdaptationRejection> {
    let admitted = check_binding(source, binding)?;
    admit_bound_edit(json, admitted)
}

fn admit_bound_edit(
    json: &str,
    admitted: BoundSource<'_>,
) -> Result<ScriptContent, AdaptationRejection> {
    let binding = admitted.facts();
    let content = read_script(json.as_bytes()).map_err(script_problem)?;
    let script: Value = serde_json::from_slice(&content.export_bytes())
        .map_err(|_| problem("invalid_output", "/", "json"))?;
    let origin = format!("origin_{}", binding.id_namespace);
    let generated = format!("generated_{}", binding.id_namespace);
    let mut expected = if let Some(base) = &binding.base_script_json {
        let base = read_script(base.as_bytes()).map_err(script_problem)?;
        let base: Value = serde_json::from_slice(&base.export_bytes())
            .map_err(|_| problem("invalid_base", "/", "json"))?;
        if script["work"]["id"] != base["work"]["id"]
            || script["work"]["source_ref"] != base["work"]["source_ref"]
            || script["work"]["rights_record_id"] != base["work"]["rights_record_id"]
            || script["adaptation"]["id"] != base["adaptation"]["id"]
            || script["episode"]["id"] != base["episode"]["id"]
        {
            return Err(problem(
                "binding_mismatch",
                "/work",
                "immutable_identity_and_evidence",
            ));
        }
        array(&base["provenance"])?.clone()
    } else {
        if script["work"]["source_ref"] != origin
            || script["work"]["rights_record_id"] != binding.rights_record_id
        {
            return Err(problem("binding_mismatch", "/work", "source_and_rights"));
        }
        vec![]
    };
    expected.push(json!({"id":origin,"kind":"imported","source_record_id":binding.source_id,"rights_record_id":binding.rights_record_id}));
    expected.push(json!({"id":generated,"kind":"generated","source_record_id":binding.source_id,"rights_record_id":binding.rights_record_id,"generation_record_id":binding.generation_record_id}));
    expected.sort_by_key(|item| item["id"].to_string());
    let mut refs: Vec<Value> = expected.iter().map(|item| item["id"].clone()).collect();
    refs.sort_by_key(Value::to_string);
    if script["provenance"] != json!(expected)
        || script["adaptation"]["rights_record_id"] != binding.rights_record_id
        || script["adaptation"]["provenance_refs"] != json!(refs)
    {
        return Err(problem(
            "binding_mismatch",
            "/provenance",
            "immutable_attempt_evidence",
        ));
    }
    Ok(content)
}
