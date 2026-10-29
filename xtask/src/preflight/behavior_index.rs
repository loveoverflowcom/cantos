//! The registered-behavior reverse dependency index: changed path -> affected capability IDs.
//!
//! `scripts/verify_behavior.py --impact-index` resolves the root registry, every module-owned
//! manifest and each Quint import closure, then prints this index. Selection only reads it: the
//! registry is the one authority, with no second capability map here. An unavailable or invalid
//! index is `None`, which keeps the conservative, index-free selection.
//!
//! Schema 2 adds `unmodeled`: live capabilities without a runnable model. Their production paths
//! select no gate; they derive an assurance obligation that needs a recorded decision.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::Command,
};

use serde::Deserialize;

use crate::gate::Gate;

/// Entry in `inputs` meaning "no single runnable capability": run every model.
const EVERY_MODEL: &str = "*";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BehaviorIndex {
    schema_version: u32,
    capabilities: BTreeSet<String>,
    observed: BTreeSet<String>,
    /// Live capabilities without a runnable model: lifecycle and registry reason.
    unmodeled: BTreeMap<String, Unmodeled>,
    /// Executable behavior files: Quint closure, shared helpers and manifests.
    inputs: BTreeMap<String, BTreeSet<String>>,
    /// Non-document production references that select the model run.
    sources: BTreeMap<String, BTreeSet<String>>,
    /// Production cores and observer tests that select the registered observer.
    conformance: BTreeMap<String, BTreeSet<String>>,
    /// Production paths of unmodeled capabilities: an obligation, never a gate.
    obligations: BTreeMap<String, BTreeSet<String>>,
    source_prefixes: BTreeMap<String, BTreeSet<String>>,
    conformance_prefixes: BTreeMap<String, BTreeSet<String>>,
    obligation_prefixes: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unmodeled {
    pub lifecycle: String,
    pub reason: String,
}

/// What one changed path means for registered behavior.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PathEffect {
    /// A registered Quint/manifest/helper file: not a Cargo input, so package classification is
    /// skipped for it.
    pub executable_input: bool,
    pub every_model: bool,
    pub models: BTreeSet<String>,
    pub conformance: BTreeSet<String>,
    /// Unmodeled capabilities whose registered production path this is.
    pub unmodeled: BTreeSet<String>,
}

/// Behavior gates accumulated over a change set.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BehaviorPlan {
    every_model: bool,
    every_observer: bool,
    models: BTreeSet<String>,
    conformance: BTreeSet<String>,
}

impl BehaviorIndex {
    pub fn parse(json: &str) -> Result<Self, String> {
        let index: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        if index.schema_version != 2 {
            return Err("unsupported behavior index schema".into());
        }
        let known = |ids: &BTreeMap<String, BTreeSet<String>>| {
            ids.values()
                .flatten()
                .all(|id| index.capabilities.contains(id) || id == EVERY_MODEL)
        };
        let tables = [&index.inputs, &index.sources, &index.conformance];
        if !tables.into_iter().all(known)
            || !known(&index.source_prefixes)
            || !known(&index.conformance_prefixes)
            || !index.observed.is_subset(&index.capabilities)
        {
            return Err("behavior index names an unregistered capability".into());
        }
        let unmodeled = |ids: &BTreeMap<String, BTreeSet<String>>| {
            ids.values()
                .flatten()
                .all(|id| index.unmodeled.contains_key(id))
        };
        if !unmodeled(&index.obligations)
            || !unmodeled(&index.obligation_prefixes)
            || index
                .unmodeled
                .keys()
                .any(|id| index.capabilities.contains(id))
        {
            return Err("behavior index obligation names a runnable or unknown capability".into());
        }
        Ok(index)
    }

    /// Ask the registry for its index. `None` when the verifier is absent, the registry is
    /// invalid or the output is unreadable: selection then stays conservative and the Behavior
    /// gate itself reports the registry failure.
    pub fn load(root: &Path) -> Option<Self> {
        if !root.join(crate::behavior::SCRIPT).is_file() {
            return None;
        }
        let output = Command::new("python3")
            .args([crate::behavior::SCRIPT, "--impact-index"])
            .current_dir(root)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Self::parse(std::str::from_utf8(&output.stdout).ok()?).ok()
    }

    pub fn effect(&self, path: &str) -> PathEffect {
        let mut effect = PathEffect::default();
        if let Some(ids) = self.inputs.get(path) {
            effect.executable_input = true;
            effect.add_models(ids);
        }
        if let Some(ids) = self.sources.get(path) {
            effect.add_models(ids);
        }
        for (prefix, ids) in &self.source_prefixes {
            if path.starts_with(prefix.as_str()) {
                effect.add_models(ids);
            }
        }
        if let Some(ids) = self.conformance.get(path) {
            effect.add_conformance(ids);
        }
        for (prefix, ids) in &self.conformance_prefixes {
            if path.starts_with(prefix.as_str()) {
                effect.add_conformance(ids);
            }
        }
        if let Some(ids) = self.obligations.get(path) {
            effect.unmodeled.extend(ids.iter().cloned());
        }
        for (prefix, ids) in &self.obligation_prefixes {
            if path.starts_with(prefix.as_str()) {
                effect.unmodeled.extend(ids.iter().cloned());
            }
        }
        effect
    }

    pub fn unmodeled(&self, id: &str) -> Option<&Unmodeled> {
        self.unmodeled.get(id)
    }
}

impl PathEffect {
    fn add_models(&mut self, ids: &BTreeSet<String>) {
        for id in ids {
            if id == EVERY_MODEL {
                self.every_model = true;
            } else {
                self.models.insert(id.clone());
            }
        }
    }

    fn add_conformance(&mut self, ids: &BTreeSet<String>) {
        // A conformance path also re-runs the model the observer replays.
        self.models.extend(ids.iter().cloned());
        self.conformance.extend(ids.iter().cloned());
    }
}

impl BehaviorPlan {
    pub fn include(&mut self, effect: PathEffect) {
        self.every_model |= effect.every_model;
        self.models.extend(effect.models);
        self.conformance.extend(effect.conformance);
    }

    /// Tooling shared by every capability: all models, and the observers when `observers`.
    pub fn include_everything(&mut self, observers: bool) {
        self.every_model = true;
        self.every_observer |= observers;
    }

    pub fn gates(&self, index: &BehaviorIndex) -> Vec<Gate> {
        let gate = |capability: Option<&str>, conformance: bool| Gate::Behavior {
            capability: capability.map(str::to_owned),
            setup: false,
            conformance,
        };
        if self.every_observer {
            return vec![gate(None, true)];
        }
        let mut gates = Vec::new();
        if self.every_model {
            gates.push(gate(None, false));
        }
        for id in &self.models {
            let observed = self.conformance.contains(id) && index.observed.contains(id);
            // The unscoped run already executes every model; only observers still need a gate.
            if observed || !self.every_model {
                gates.push(gate(Some(id), observed));
            }
        }
        gates.sort();
        gates
    }
}

#[cfg(test)]
pub mod testing {
    use super::BehaviorIndex;

    /// A synthetic federated registry whose manifests and models live in module-owned trees:
    /// two plain capabilities, one observed capability, one shared helper, one legacy inline
    /// historical scenario ("*" = no single runnable capability) and one unmodeled capability.
    pub fn federated() -> BehaviorIndex {
        BehaviorIndex::parse(
            r#"{
              "schemaVersion": 2,
              "capabilities": ["cantos.a.one", "cantos.b.two", "cantos.c.observed"],
              "observed": ["cantos.c.observed"],
              "unmodeled": {
                "cantos.d.unmodeled": {"lifecycle": "draft", "reason": "core exists; model deferred"}
              },
              "inputs": {
                "services/cantos-forge-functions/behavior/one.qnt": ["cantos.a.one"],
                "services/cantos-forge-functions/behavior/one_tests.qnt": ["cantos.a.one"],
                "services/cantos-forge-functions/behavior/one.manifest.json": ["cantos.a.one"],
                "services/cantos-server/behavior/two.qnt": ["cantos.b.two"],
                "xtask/behavior/hosting.qnt": ["cantos.b.two"],
                "specs/behavior/shared/clock.qnt": ["cantos.a.one", "cantos.b.two"],
                "specs/behavior/legacy/old_tests.qnt": ["*"],
                "services/cantos-identity/behavior/c.qnt": ["cantos.c.observed"]
              },
              "sources": {
                "services/cantos-forge-functions/src/domain/attempt.rs": ["cantos.a.one"],
                "services/cantos-server/src/contexts/media/two.rs": ["cantos.b.two"]
              },
              "conformance": {
                "services/cantos-identity/src/domain/ids.rs": ["cantos.c.observed"],
                "services/cantos-identity/src/observer_tests.rs": ["cantos.c.observed"]
              },
              "obligations": {
                "services/cantos-server/src/contexts/media/domain/publication.rs": ["cantos.d.unmodeled"]
              },
              "sourcePrefixes": {"services/cantos-forge-functions/src/owned/": ["cantos.a.one"]},
              "conformancePrefixes": {"services/cantos-identity/src/kernel/": ["cantos.c.observed"]},
              "obligationPrefixes": {"services/cantos-server/src/contexts/listening/": ["cantos.d.unmodeled"]}
            }"#,
        )
        .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::federated, BehaviorIndex};

    #[test]
    fn schema_two_obligations_name_only_unmodeled_capabilities() {
        let index = federated();
        let effect =
            index.effect("services/cantos-server/src/contexts/media/domain/publication.rs");
        assert_eq!(effect.unmodeled, ["cantos.d.unmodeled".to_owned()].into());
        assert!(
            effect.models.is_empty() && !effect.every_model,
            "an obligation is never a gate"
        );
        assert_eq!(
            index
                .effect("services/cantos-server/src/contexts/listening/domain/x.rs")
                .unmodeled,
            ["cantos.d.unmodeled".to_owned()].into()
        );
        assert_eq!(
            index.unmodeled("cantos.d.unmodeled").unwrap().lifecycle,
            "draft"
        );
        let minimal = |schema: u32, unmodeled: &str, obligations: &str| {
            format!(
                r#"{{"schemaVersion": {schema}, "capabilities": ["cantos.a.one"], "observed": [],
                    "unmodeled": {unmodeled}, "inputs": {{}}, "sources": {{}}, "conformance": {{}},
                    "obligations": {obligations}, "sourcePrefixes": {{}}, "conformancePrefixes": {{}},
                    "obligationPrefixes": {{}}}}"#
            )
        };
        let unmodeled = r#"{"cantos.d.x": {"lifecycle": "draft", "reason": "r"}}"#;
        assert!(
            BehaviorIndex::parse(&minimal(2, unmodeled, r#"{"a.rs": ["cantos.d.x"]}"#)).is_ok()
        );
        for (schema, unmodeled, obligations) in [
            (1, unmodeled, "{}"),
            (2, unmodeled, r#"{"a.rs": ["cantos.a.one"]}"#),
            (2, unmodeled, r#"{"a.rs": ["cantos.unknown"]}"#),
            (
                2,
                r#"{"cantos.a.one": {"lifecycle": "draft", "reason": "r"}}"#,
                "{}",
            ),
        ] {
            assert!(
                BehaviorIndex::parse(&minimal(schema, unmodeled, obligations)).is_err(),
                "{schema} {unmodeled} {obligations}"
            );
        }
    }
}
