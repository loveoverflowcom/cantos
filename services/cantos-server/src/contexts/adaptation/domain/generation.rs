//! Caller generation declarations: validated for shape and bounds, never attested.
use crate::contexts::editorial::domain::diagnostics::SafeDiagnostic;
use crate::contract::studio::{AdaptationCostBasis, CallerGenerationMetadata};
use serde_json::Value;

pub const MAX_CALLER_CONFIGURATION_BYTES: usize = 16 * 1024;
const MAX_CALLER_CONFIGURATION_DEPTH: usize = 8;
const MAX_CALLER_CONFIGURATION_NODES: usize = 256;

/// Validate declarations without attesting them or inventing missing provider/accounting facts.
pub fn validate_caller_generation(
    metadata: &CallerGenerationMetadata,
    pinned_prompt_version: &str,
) -> Vec<SafeDiagnostic> {
    let mut issues = Vec::new();
    for (value, max, path) in [
        (
            Some(metadata.host_tool.as_str()),
            128,
            "/generation/host_tool",
        ),
        (metadata.provider.as_deref(), 128, "/generation/provider"),
        (metadata.model.as_deref(), 256, "/generation/model"),
        (
            Some(metadata.prompt_version.as_str()),
            128,
            "/generation/prompt_version",
        ),
    ] {
        if value.is_some_and(|value| {
            value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control)
        }) {
            issues.push(SafeDiagnostic {
                path: path.into(),
                rule: "bounded_declaration",
            });
        }
    }
    if metadata.prompt_version != pinned_prompt_version {
        issues.push(SafeDiagnostic {
            path: "/generation/prompt_version".into(),
            rule: "pinned_prompt_version",
        });
    }
    if let Some(configuration) = &metadata.configuration_json {
        if let Some(rule) = configuration_problem(configuration) {
            issues.push(SafeDiagnostic {
                path: "/generation/configuration_json".into(),
                rule,
            });
        }
    }
    if metadata
        .cost
        .as_ref()
        .is_some_and(|cost| cost.basis != AdaptationCostBasis::CallerDeclared)
    {
        issues.push(SafeDiagnostic {
            path: "/generation/cost/basis".into(),
            rule: "caller_declared",
        });
    }
    issues
}

fn configuration_problem(configuration: &str) -> Option<&'static str> {
    if configuration.len() > MAX_CALLER_CONFIGURATION_BYTES {
        return Some("byte_length");
    }
    let value: Value = match serde_json::from_str(configuration) {
        Ok(Value::Object(object)) => Value::Object(object),
        _ => return Some("json_object"),
    };
    let mut nodes = 0usize;
    let mut pending = vec![(&value, 1usize)];
    while let Some((value, depth)) = pending.pop() {
        nodes = nodes.saturating_add(1);
        if nodes > MAX_CALLER_CONFIGURATION_NODES {
            return Some("bounded_json_object");
        }
        match value {
            Value::Object(object) => {
                nodes = nodes.saturating_add(object.len()); // Keys are bounded nodes too.
                if depth > MAX_CALLER_CONFIGURATION_DEPTH || nodes > MAX_CALLER_CONFIGURATION_NODES
                {
                    return Some("bounded_json_object");
                }
                pending.extend(object.values().map(|value| (value, depth + 1)));
            }
            Value::Array(array) => {
                if depth > MAX_CALLER_CONFIGURATION_DEPTH
                    || array.len() > MAX_CALLER_CONFIGURATION_NODES.saturating_sub(nodes)
                {
                    return Some("bounded_json_object");
                }
                pending.extend(array.iter().map(|value| (value, depth + 1)));
            }
            _ => {}
        }
    }
    None
}
