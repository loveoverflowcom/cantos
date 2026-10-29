//! Durable fixture recovery only. Expiration never mutates an old intent or implies that an
//! upload failed: the Server must first report that exact intent expired with no admitted asset.
use std::{
    fs,
    path::{Path, PathBuf},
};

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{http, input, no_symlink, private_dir, sample_files, RIGHTS};

const MAX_ATTEMPTS: usize = 8;

#[derive(Debug, PartialEq, Eq)]
pub enum RecoveryError {
    Journal,
    Binding,
    AttemptLimit,
    ServerStatus(u16),
    StoredState,
}

pub struct Plan<'a> {
    pub key: &'a str,
    pub work_id: &'a str,
    pub input_sha256: &'a str,
    pub provenance: &'a str,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Attempt {
    request: Value,
    expired_observation: Option<Value>,
    asset: Option<Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    binding: Value,
    attempts: Vec<Attempt>,
}

pub struct Journal {
    path: PathBuf,
    document: Document,
    persisted: Option<Vec<u8>>,
    expected_asset: Option<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObservationState {
    Pending,
    Admitted,
    Expired,
}

/// Live and restored observations cross the same required-field and scope boundary.
/// An omitted nullable field is not evidence that the Server observed no admitted asset.
fn observation_state(
    binding: &Value,
    request: &Value,
    value: &Value,
) -> Result<ObservationState, RecoveryError> {
    let intent = value
        .get("intent")
        .and_then(Value::as_object)
        .ok_or(RecoveryError::Binding)?;
    if intent.get("request") != Some(request)
        || intent.get("work_id") != binding.get("work_id")
        || intent.get("id") != request.get("id")
    {
        return Err(RecoveryError::Binding);
    }
    let state = value
        .get("state")
        .and_then(Value::as_str)
        .ok_or(RecoveryError::StoredState)?;
    let asset = value.get("asset").ok_or(RecoveryError::StoredState)?;
    match (state, asset) {
        ("pending", Value::Null) => Ok(ObservationState::Pending),
        ("expired", Value::Null) => Ok(ObservationState::Expired),
        ("admitted", Value::Object(_)) => Ok(ObservationState::Admitted),
        _ => Err(RecoveryError::StoredState),
    }
}

fn request(binding: &Value, index: usize) -> Value {
    let key = binding["sample_key"].as_str().unwrap();
    let (id, operation) = if index == 0 {
        (
            input::identity(key, "image"),
            input::identity(key, "prepare-image"),
        )
    } else {
        (
            input::identity(key, &format!("image-attempt-{index}")),
            input::identity(key, &format!("prepare-image-attempt-{index}")),
        )
    };
    json!({"id":id,"operation_id":operation,"expected_sha256":binding["cover_sha256"],"expected_byte_length":binding["cover_byte_length"],"content_type":"image/avif",
        "declarations":{"provenance":binding["provenance"],"rights_refs":[RIGHTS]}})
}

impl Journal {
    pub fn open(
        root: &Path,
        plan: Plan<'_>,
        cover: &input::CoverBytes,
    ) -> Result<Self, RecoveryError> {
        assert!(
            !plan.key.is_empty()
                && plan
                    .key
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        );
        let directory = root.join("image-attempts");
        private_dir(&directory);
        let path = directory.join(format!("{}.json", plan.key));
        no_symlink(&path);
        let binding = json!({"dataset_id":input::DATASET,"root":root,"sample_key":plan.key,"work_id":plan.work_id,"input_sha256":plan.input_sha256,
            "cover_sha256":cover.sha256,"cover_byte_length":cover.bytes.len(),"provenance":plan.provenance});
        let expected_asset = if root.join("manifest.json").exists() {
            let manifest: Value = serde_json::from_slice(
                &fs::read(root.join("manifest.json")).map_err(|_| RecoveryError::Journal)?,
            )
            .map_err(|_| RecoveryError::Journal)?;
            Some(
                manifest["works"]
                    .as_array()
                    .ok_or(RecoveryError::Journal)?
                    .iter()
                    .find(|work| work["sample_key"] == plan.key)
                    .ok_or(RecoveryError::Journal)?["cover"]["asset"]
                    .clone(),
            )
        } else {
            None
        };
        let persisted = if path.exists() {
            Some(fs::read(&path).map_err(|_| RecoveryError::Journal)?)
        } else {
            None
        };
        let document = if let Some(bytes) = &persisted {
            let document: Document =
                serde_json::from_slice(bytes).map_err(|_| RecoveryError::Journal)?;
            if document.binding != binding {
                return Err(RecoveryError::Binding);
            }
            if document.attempts.is_empty() || document.attempts.len() > MAX_ATTEMPTS {
                return Err(RecoveryError::Journal);
            }
            for (index, attempt) in document.attempts.iter().enumerate() {
                if attempt.request != request(&binding, index) {
                    return Err(RecoveryError::Binding);
                }
                if index + 1 < document.attempts.len()
                    && (attempt.expired_observation.is_none() || attempt.asset.is_some())
                {
                    return Err(RecoveryError::Journal);
                }
                if let Some(observation) = &attempt.expired_observation {
                    match observation_state(&binding, &attempt.request, observation) {
                        Ok(ObservationState::Expired) => {}
                        Err(RecoveryError::Binding) => return Err(RecoveryError::Binding),
                        Ok(_) | Err(_) => return Err(RecoveryError::Journal),
                    }
                }
            }
            if document
                .attempts
                .last()
                .unwrap()
                .expired_observation
                .is_some()
            {
                return Err(RecoveryError::Journal);
            }
            document
        } else {
            Document {
                attempts: vec![Attempt {
                    request: request(&binding, 0),
                    expired_observation: None,
                    asset: None,
                }],
                binding,
            }
        };
        if expected_asset
            .as_ref()
            .is_some_and(|asset| asset["id"] != document.attempts.last().unwrap().request["id"])
        {
            return Err(RecoveryError::Journal);
        }
        let mut journal = Self {
            path,
            document,
            persisted,
            expected_asset,
        };
        journal.save()?;
        Ok(journal)
    }

    fn save(&mut self) -> Result<(), RecoveryError> {
        let bytes =
            serde_json::to_vec_pretty(&self.document).map_err(|_| RecoveryError::Journal)?;
        match &self.persisted {
            Some(prior) => sample_files::replace_owned(&self.path, prior, &bytes),
            None => sample_files::write_once(&self.path, &bytes),
        }
        .map_err(|_| RecoveryError::Journal)?;
        self.persisted = Some(bytes);
        Ok(())
    }

    pub fn current_request(&self) -> &Value {
        &self.document.attempts.last().unwrap().request
    }

    fn observe(&self, value: &Value) -> Result<ObservationState, RecoveryError> {
        observation_state(&self.document.binding, self.current_request(), value)
    }

    fn expired(&mut self, observation: Value) -> Result<(), RecoveryError> {
        let state = self.observe(&observation)?;
        let current = self.document.attempts.last().unwrap();
        if state != ObservationState::Expired
            || current.asset.is_some()
            || self.expected_asset.is_some()
        {
            return Err(RecoveryError::StoredState);
        }
        if self.document.attempts.len() >= MAX_ATTEMPTS {
            return Err(RecoveryError::AttemptLimit);
        }
        let next = request(&self.document.binding, self.document.attempts.len());
        self.document
            .attempts
            .last_mut()
            .unwrap()
            .expired_observation = Some(observation);
        self.document.attempts.push(Attempt {
            request: next,
            expired_observation: None,
            asset: None,
        });
        self.save()
    }

    fn admitted(
        &mut self,
        asset: Value,
        cover: &input::CoverBytes,
    ) -> Result<Value, RecoveryError> {
        if self
            .expected_asset
            .as_ref()
            .is_some_and(|expected| expected != &asset)
        {
            return Err(RecoveryError::StoredState);
        }
        if asset["id"] != self.current_request()["id"]
            || asset["work_id"] != self.document.binding["work_id"]
            || asset["verified_media"]["sha256"] != cover.sha256
            || asset["verified_media"]["byte_length"] != cover.bytes.len()
            || asset["verified_media"]["width_px"] != cover.width
            || asset["verified_media"]["height_px"] != cover.height
        {
            return Err(RecoveryError::Binding);
        }
        let current = self.document.attempts.last_mut().unwrap();
        if current.expired_observation.is_some()
            || current.asset.as_ref().is_some_and(|prior| prior != &asset)
        {
            return Err(RecoveryError::StoredState);
        }
        current.asset = Some(asset.clone());
        self.save()?;
        Ok(asset)
    }

    pub async fn admit(
        &mut self,
        host: &http::HttpHost,
        token: &str,
        cover: &input::CoverBytes,
    ) -> Result<Value, RecoveryError> {
        // Two observations per attempt allow preparation and one expiry race reconciliation.
        // A timeout or any unrecognized result preserves this attempt for the next invocation.
        for _ in 0..MAX_ATTEMPTS * 3 {
            let body = self.current_request().clone();
            let work = self.document.binding["work_id"].as_str().unwrap();
            let path = format!(
                "/api/v1/studio/works/{work}/image-intents/{}",
                body["id"].as_str().unwrap()
            );
            let reply = host
                .json::<Value>("GET", &path, Some(token), None, None)
                .await;
            if reply.status == StatusCode::NOT_FOUND {
                reply.error(StatusCode::NOT_FOUND, "not_found");
                if self.document.attempts.last().unwrap().asset.is_some() {
                    return Err(RecoveryError::StoredState);
                }
                let prepared = host
                    .json(
                        "POST",
                        &format!("/api/v1/studio/works/{work}/image-intents"),
                        Some(token),
                        None,
                        Some(&body),
                    )
                    .await;
                if prepared.status != StatusCode::CREATED {
                    return Err(RecoveryError::ServerStatus(prepared.status.as_u16()));
                }
                let document: Value = prepared.success(StatusCode::CREATED);
                if document["request"] != body
                    || document.get("id") != body.get("id")
                    || document.get("work_id") != self.document.binding.get("work_id")
                {
                    return Err(RecoveryError::Binding);
                }
                continue;
            }
            if reply.status != StatusCode::OK {
                return Err(RecoveryError::ServerStatus(reply.status.as_u16()));
            }
            let observed: Value = reply.success(StatusCode::OK);
            let state = self.observe(&observed)?;
            if state == ObservationState::Admitted {
                return self.admitted(observed["asset"].clone(), cover);
            }
            if state == ObservationState::Expired {
                self.expired(observed)?;
                continue;
            }
            if self.document.attempts.last().unwrap().asset.is_some() {
                return Err(RecoveryError::StoredState);
            }
            let uploaded = http::request(
                &host.client,
                &host.base,
                "PUT",
                &format!("{path}/bytes"),
                Some(token),
                None,
                Some("image/avif"),
                cover.bytes.clone(),
                &[],
            )
            .await;
            if uploaded.status == StatusCode::OK {
                return self.admitted(uploaded.success(StatusCode::OK), cover);
            }
            let failure: Value =
                serde_json::from_slice(&uploaded.bytes).map_err(|_| RecoveryError::StoredState)?;
            if uploaded.status == StatusCode::CONFLICT
                && failure["issues"].as_array().is_some_and(|issues| {
                    issues
                        .iter()
                        .any(|issue| issue["path"] == "intent" && issue["rule"] == "intent_expired")
                })
            {
                continue;
            }
            return Err(RecoveryError::ServerStatus(uploaded.status.as_u16()));
        }
        Err(RecoveryError::AttemptLimit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn try_open(root: &Path) -> Result<Journal, RecoveryError> {
        let bytes =
            include_bytes!("../../src/contexts/media/image_intake/fixtures/python-rgb-2x3.avif")
                .to_vec();
        let cover = input::CoverBytes {
            sha256: input::hash(&bytes),
            bytes,
            width: 2,
            height: 3,
        };
        Journal::open(
            root,
            Plan {
                key: "ben-suong",
                work_id: "00000000-0000-4000-8000-000000000001",
                input_sha256: "synthetic-input",
                provenance: "Synthetic pixels",
            },
            &cover,
        )
    }

    fn open(root: &Path) -> Journal {
        try_open(root).unwrap()
    }

    fn expired(journal: &Journal) -> Value {
        json!({"state":"expired","asset":null,"intent":{"id":journal.current_request()["id"],"work_id":journal.document.binding["work_id"],"request":journal.current_request()}})
    }

    #[test]
    fn only_bound_expired_nonadmitted_observation_advances_and_attempts_are_bounded() {
        let run = cantos_store_turso::testing::TestRun::new("sample-image-journal");
        let root = fs::canonicalize(run.dir()).unwrap();
        let mut journal = open(&root);
        let mut wrong = expired(&journal);
        wrong["intent"]["work_id"] = json!("different-work");
        assert_eq!(journal.expired(wrong), Err(RecoveryError::Binding));
        let mut accepted = expired(&journal);
        accepted["asset"] = json!({"id":journal.current_request()["id"]});
        assert_eq!(journal.expired(accepted), Err(RecoveryError::StoredState));
        let initial_bytes = fs::read(&journal.path).unwrap();
        let initial_request = journal.current_request().clone();
        for field in ["asset", "state"] {
            let mut missing = expired(&journal);
            missing.as_object_mut().unwrap().remove(field);
            assert_eq!(journal.expired(missing), Err(RecoveryError::StoredState));
            assert_eq!(journal.current_request(), &initial_request);
            assert_eq!(fs::read(&journal.path).unwrap(), initial_bytes);
        }
        for _ in 1..MAX_ATTEMPTS {
            journal.expired(expired(&journal)).unwrap();
            let current = journal.current_request().clone();
            journal = open(&root);
            assert_eq!(journal.current_request(), &current);
        }
        let before = fs::read(&journal.path).unwrap();
        assert_eq!(
            journal.expired(expired(&journal)),
            Err(RecoveryError::AttemptLimit)
        );
        assert_eq!(fs::read(&journal.path).unwrap(), before);
    }

    #[test]
    fn restored_expired_observations_require_exact_intent_scope_and_explicit_null_asset() {
        let run = cantos_store_turso::testing::TestRun::new("sample-image-observation-restore");
        let root = fs::canonicalize(run.dir()).unwrap();
        let mut journal = open(&root);
        journal.expired(expired(&journal)).unwrap();
        let original = fs::read(&journal.path).unwrap();
        for (field, replacement, expected) in [
            (
                "id",
                Some(json!("different-intent")),
                RecoveryError::Binding,
            ),
            (
                "work_id",
                Some(json!("different-work")),
                RecoveryError::Binding,
            ),
            ("id", None, RecoveryError::Binding),
            ("work_id", None, RecoveryError::Binding),
            ("asset", None, RecoveryError::Journal),
            ("state", None, RecoveryError::Journal),
        ] {
            let mut document: Value = serde_json::from_slice(&original).unwrap();
            let observation = &mut document["attempts"][0]["expired_observation"];
            let object = if matches!(field, "id" | "work_id") {
                observation["intent"].as_object_mut().unwrap()
            } else {
                observation.as_object_mut().unwrap()
            };
            match replacement {
                Some(value) => {
                    object.insert(field.into(), value);
                }
                None => {
                    object.remove(field);
                }
            }
            let corrupted = serde_json::to_vec_pretty(&document).unwrap();
            sample_files::replace_owned(&journal.path, &original, &corrupted).unwrap();
            assert_eq!(try_open(&root).err(), Some(expected));
            assert_eq!(
                fs::read(&journal.path).unwrap(),
                corrupted,
                "refused restoration must preserve the durable record"
            );
            sample_files::replace_owned(&journal.path, &corrupted, &original).unwrap();
        }
        assert_eq!(open(&root).current_request(), journal.current_request());
    }

    #[test]
    fn missing_journal_for_nonlegacy_manifested_asset_refuses_to_restart_at_zero() {
        let run = cantos_store_turso::testing::TestRun::new("sample-image-missing-journal");
        let root = fs::canonicalize(run.dir()).unwrap();
        let manifest = json!({"works":[{"sample_key":"ben-suong","cover":{"asset":{"id":input::identity("ben-suong","image-attempt-1")}}}]});
        sample_files::write_once(
            &root.join("manifest.json"),
            &serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let bytes =
            include_bytes!("../../src/contexts/media/image_intake/fixtures/python-rgb-2x3.avif")
                .to_vec();
        let cover = input::CoverBytes {
            sha256: input::hash(&bytes),
            bytes,
            width: 2,
            height: 3,
        };
        assert!(matches!(
            Journal::open(
                &root,
                Plan {
                    key: "ben-suong",
                    work_id: "00000000-0000-4000-8000-000000000001",
                    input_sha256: "synthetic-input",
                    provenance: "Synthetic pixels"
                },
                &cover
            ),
            Err(RecoveryError::Journal)
        ));
        assert!(!root.join("image-attempts/ben-suong.json").exists());
    }
}
