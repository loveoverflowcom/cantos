//! Pure versioned request plan. The expected final rows are built from fixture intent, never
//! from a SQL/HTTP response. No production schema or credential belongs in this module.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const REGISTRY: &str = include_str!("../../fixtures/local-datasets/registry.json");
pub const CORPUS: &str = include_str!("../../fixtures/local-datasets/substring.json");
pub const DATASET: &str = "cantos-local-datasets-v1";
pub const CLOCK: i64 = 1_800_000_000_000_000;
pub const ACTORS: [&str; 5] = [
    "dataset_owner_a",
    "dataset_owner_b",
    "dataset_editor",
    "dataset_reader",
    "dataset_stranger",
];

pub fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn identity(profile: &str, seed: u64, purpose: &str) -> String {
    let digest = Sha256::digest(format!("{DATASET}\0{profile}\0{seed}\0{purpose}"));
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 15) | 0x50;
    bytes[8] = (bytes[8] & 63) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Step {
    pub id: String,
    pub actor: usize,
    pub method: String,
    pub path: String,
    pub body: Value,
    pub status: u16,
    pub code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Unit {
    pub id: String,
    pub title: String,
    pub description: String,
    pub position: usize,
    pub revision: u64,
    pub archived: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Work {
    pub id: String,
    pub actor: usize,
    pub kind: String,
    pub title: String,
    pub synopsis: String,
    pub language: String,
    pub visibility: String,
    pub revision: u64,
    pub archived: bool,
    pub units: Vec<Unit>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub version: u32,
    pub profile: String,
    pub seed: u64,
    pub size: usize,
    pub clock_us: i64,
    pub works: Vec<Work>,
    pub steps: Vec<Step>,
}
impl Plan {
    pub fn build(profile: &str, seed: u64, size: Option<usize>) -> Result<Self, String> {
        let registry: Value = serde_json::from_str(REGISTRY).map_err(|e| e.to_string())?;
        let entry = registry["profiles"]
            .as_array()
            .ok_or("profile registry")?
            .iter()
            .find(|p| p["name"] == profile)
            .ok_or("unknown dataset profile")?;
        let size = size.unwrap_or(entry["default_size"].as_u64().ok_or("default size")? as usize);
        if !entry["sizes"]
            .as_array()
            .ok_or("sizes")?
            .contains(&json!(size))
        {
            return Err("size is outside this profile's explicit bounds".into());
        }
        let mut p = Self {
            version: 1,
            profile: profile.into(),
            seed,
            size,
            clock_us: CLOCK,
            works: vec![],
            steps: vec![],
        };
        let count = match profile {
            "empty" | "vector-corpus" => 0,
            "minimal" | "studio-crud" => 1,
            "relationships-capacity" => 3,
            _ => size,
        };
        let corpus: Value = serde_json::from_str(CORPUS).map_err(|e| e.to_string())?;
        for i in 0..count {
            let mut w = Work {
                id: p.id(&format!("work/{i}")),
                actor: usize::from(profile == "permissions-conflicts" && i == 1),
                kind: ["novel", "story", "podcast"][i % 3].into(),
                title: format!("Bến thử nghiệm {:03}", i % 7),
                synopsis: "Dữ liệu tổng hợp riêng tư; chưa có âm thanh.".into(),
                language: if i % 2 == 0 { "vi-VN" } else { "en" }.into(),
                visibility: "private".into(),
                revision: 1,
                archived: false,
                units: vec![],
            };
            if matches!(
                profile,
                "unicode-and-patch" | "substring-search" | "fts-corpus"
            ) {
                let d = &corpus["documents"][i];
                w.kind = d["kind"].as_str().ok_or("kind")?.into();
                w.title = d["title"].as_str().ok_or("title")?.into();
                w.synopsis = d["description"].as_str().ok_or("description")?.into();
                w.language = d["language"].as_str().ok_or("language")?.into();
            }
            if profile == "visibility-recovery" {
                w.visibility = ["private", "unlisted", "public"][i].into();
            }
            p.push(w.actor,"POST","/works".into(),json!({"id":w.id,"kind":w.kind,"details":{"title":w.title,"synopsis":w.synopsis,"language":w.language,"visibility":w.visibility,"rights_refs":[]}}),201,None);
            let children = match profile {
                "minimal" => size,
                "studio-crud" => 2,
                "relationships-capacity" => {
                    if i == 0 {
                        0
                    } else if i == 1 {
                        1
                    } else {
                        size
                    }
                }
                "mixed-kinds" => [0, 1, 3][i % 3],
                "scale-local" => {
                    if i % 100 == 0 {
                        10
                    } else {
                        usize::from(i % 10 == 0)
                    }
                }
                "unicode-and-patch" => usize::from(i == 0),
                _ => 0,
            };
            for j in 0..children {
                let u = Unit {
                    id: p.id(&format!("work/{i}/unit/{j}")),
                    title: format!("Chương hoặc tập {j} — Mưa về"),
                    description: String::new(),
                    position: j,
                    revision: 1,
                    archived: false,
                };
                p.push(w.actor,"POST",format!("/works/{}/content-units",w.id),json!({"id":u.id,"kind":if w.kind=="podcast" {"episode"} else {"chapter"},"expected_work_revision":w.revision,"details":{"title":u.title,"description":u.description}}),201,None);
                w.revision += 1;
                w.units.push(u);
            }
            p.works.push(w);
        }
        if profile == "mixed-kinds" {
            p.archive_work(count - 1);
        }
        if profile == "relationships-capacity" && size > 0 {
            p.archive_unit(2, 0);
        }
        if profile == "studio-crud" {
            p.patch_work(0, json!({"title":"Tác phẩm đã sửa","synopsis":""}));
            let w = p.works[0].clone();
            p.push(0,"PATCH",format!("/content-units/{}",w.units[0].id),json!({"expected_revision":1,"expected_work_revision":w.revision,"details":{"description":"Dòng một\nDòng hai","source":null,"script":null,"content_digest":null}}),200,None);
            p.works[0].revision += 1;
            p.works[0].units[0].revision += 1;
            p.works[0].units[0].description = "Dòng một\nDòng hai".into();
            let w = p.works[0].clone();
            p.push(
                0,
                "POST",
                format!("/works/{}/content-units/reorder", w.id),
                json!({"expected_revision":w.revision,"ordered":[w.units[1].id,w.units[0].id]}),
                200,
                None,
            );
            p.works[0].revision += 1;
            for u in &mut p.works[0].units {
                u.position = 1 - u.position;
                u.revision += 1;
            }
            p.archive_unit(0, 0);
            p.archive_work(0);
        }
        if profile == "unicode-and-patch" {
            p.patch_work(
                0,
                json!({"title":"é".repeat(256),"synopsis":"é".repeat(8192)}),
            );
            p.patch_work(0, json!({"synopsis":""}));
        }
        if profile == "permissions-conflicts" {
            for (actor, role) in [(2, "editor"), (3, "reader")] {
                let w = p.works[0].clone();
                p.push(
                    0,
                    "MEMBER",
                    w.id.clone(),
                    json!({"expected_revision":w.revision,"actor_id":ACTORS[actor],"role":role}),
                    200,
                    None,
                );
                p.works[0].revision += 1;
            }
        }
        Ok(p)
    }
    pub fn id(&self, purpose: &str) -> String {
        identity(&self.profile, self.seed, purpose)
    }
    fn push(
        &mut self,
        actor: usize,
        method: &str,
        path: String,
        mut body: Value,
        status: u16,
        code: Option<&str>,
    ) {
        let id = self.id(&format!("request/{}", self.steps.len()));
        body["operation_id"] = json!(id);
        self.steps.push(Step {
            id,
            actor,
            method: method.into(),
            path,
            body,
            status,
            code: code.map(str::to_owned),
        });
    }
    fn patch_work(&mut self, i: usize, patch: Value) {
        let w = self.works[i].clone();
        self.push(
            w.actor,
            "PATCH",
            format!("/works/{}", w.id),
            json!({"expected_revision":w.revision,"details":patch}),
            200,
            None,
        );
        let w = &mut self.works[i];
        w.revision += 1;
        if let Some(s) = patch["title"].as_str() {
            w.title = s.into();
        }
        if let Some(s) = patch["synopsis"].as_str() {
            w.synopsis = s.into();
        }
    }
    fn archive_unit(&mut self, i: usize, j: usize) {
        let w = self.works[i].clone();
        let u = &w.units[j];
        self.push(
            w.actor,
            "DELETE",
            format!("/content-units/{}", u.id),
            json!({"expected_work_revision":w.revision,"expected_revision":u.revision}),
            200,
            None,
        );
        self.works[i].revision += 1;
        self.works[i].units[j].revision += 1;
        self.works[i].units[j].archived = true;
    }
    fn archive_work(&mut self, i: usize) {
        let w = self.works[i].clone();
        self.push(
            w.actor,
            "DELETE",
            format!("/works/{}", w.id),
            json!({"expected_revision":w.revision}),
            200,
            None,
        );
        self.works[i].revision += 1;
        self.works[i].archived = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_profile_has_bounded_deterministic_namespaced_requests() {
        let registry: Value = serde_json::from_str(REGISTRY).unwrap();
        assert_eq!(registry["profiles"].as_array().unwrap().len(), 13);
        for entry in registry["profiles"].as_array().unwrap() {
            let name = entry["name"].as_str().unwrap();
            let p = Plan::build(name, 568, None).unwrap();
            assert_eq!(p, Plan::build(name, 568, None).unwrap());
            assert_ne!(
                p.id("work/0"),
                Plan::build(name, 569, None).unwrap().id("work/0")
            );
            let ids: std::collections::BTreeSet<_> = p.steps.iter().map(|s| &s.id).collect();
            assert_eq!(ids.len(), p.steps.len());
            assert!(Plan::build(name, 568, Some(10001)).is_err());
        }
        assert!(Plan::build("unknown", 568, None).is_err());
        assert_eq!(
            identity("minimal", 568, "work/0"),
            "da071fb9-fe3a-5bf6-ae0b-6b36057ec04f"
        );
        assert_ne!(
            identity("empty", 568, "work/0"),
            identity("minimal", 568, "work/0")
        );
    }
    #[test]
    fn capacity_counts_retained_tombstones_and_byte_boundaries_are_literal() {
        let p = Plan::build("relationships-capacity", 568, Some(1000)).unwrap();
        assert_eq!(p.works[2].units.len(), 1000);
        assert_eq!(p.works[2].units.iter().filter(|u| !u.archived).count(), 999);
        let p = Plan::build("unicode-and-patch", 568, None).unwrap();
        assert_eq!(p.works[0].title.len(), 512);
        assert!(p.steps.iter().any(|s| s.body["details"]["synopsis"]
            .as_str()
            .is_some_and(|v| v.len() == 16384)));
    }
}
