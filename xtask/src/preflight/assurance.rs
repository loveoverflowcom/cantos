//! Assurance obligations of a change set and the decision record that answers them.
//!
//! Obligations are derived, never declared here: the behavior registry's reverse index names
//! runnable models and unmodeled capabilities, `kani::affected` names registered kernels, and an
//! uncertain selection is one `impact:unknown` obligation. A registered verifier planned in the
//! same run (a Behavior gate, an opted-in Kani gate) answers its obligation by its own result.
//! Every other obligation needs a decision in an explicit `--assurance-decisions` record.
//!
//! Two claims stay separate: validating the record (scope, authority, applicability, deferral)
//! and the verifier results it cites, which come only from gates of this run or from a
//! `kani-receipt` gate that re-validates a retained run. A status written in the record is never
//! evidence, and path mapping never proves the absence of semantic impact.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::Deserialize;

use crate::{
    gate::Gate,
    kani::{self, Kernel},
    report::{Report, Status},
};

use super::{behavior_index::BehaviorIndex, select::Selection};

pub const STEP: &str = "assurance-decisions";
pub const UNKNOWN: &str = "impact:unknown";
const MAX_RECORD_BYTES: u64 = 256 * 1024;
const ISSUE_AUTHORITY: &str = "https://github.com/loveoverflowcom/cantos/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obligation {
    /// `behavior:<id>`, `unmodeled:<id>`, `kani:<kernel>` or `impact:unknown`.
    pub id: String,
    /// The changed paths that derive it; a decision's `changedScope` must cover them.
    pub triggers: BTreeSet<String>,
    pub detail: String,
    /// A registered verifier planned in this run that answers the obligation by its result.
    pub verifier: Option<Gate>,
}

/// Documents never derive an obligation; contract fixtures are inputs, not documents.
fn document(path: &str) -> bool {
    (path.ends_with(".md") && !path.contains("/fixtures/"))
        || [".agents", ".claude", "docs", "templates"]
            .iter()
            .any(|dir| super::select::within(path, dir))
}

/// The obligations of a changed-only selection, ordered by identity.
pub fn derive(
    paths: &[String],
    selection: &Selection,
    index: Option<&BehaviorIndex>,
) -> Vec<Obligation> {
    let code: Vec<&String> = paths.iter().filter(|path| !document(path)).collect();
    let unknown = |detail: String, triggers: BTreeSet<String>| {
        vec![Obligation {
            id: UNKNOWN.into(),
            triggers,
            detail,
            verifier: None,
        }]
    };
    if let Some(reason) = &selection.broadened {
        return unknown(
            format!(
                "selection broadened ({reason}); path mapping cannot name the affected obligations"
            ),
            code.iter().map(|path| path.to_string()).collect(),
        );
    }
    let Some(index) = index else {
        if code.is_empty() {
            return Vec::new();
        }
        return unknown(
            "behavior registry index unavailable; unmodeled obligations cannot be derived".into(),
            code.iter().map(|path| path.to_string()).collect(),
        );
    };
    let planned_models: BTreeSet<&str> = selection
        .gates
        .iter()
        .filter_map(|gate| match gate {
            Gate::Behavior {
                capability: Some(id),
                ..
            } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    let mut obligations: BTreeMap<String, Obligation> = BTreeMap::new();
    let mut add = |id: String, path: Option<&str>, detail: String, verifier: Option<Gate>| {
        let entry = obligations.entry(id.clone()).or_insert(Obligation {
            id,
            triggers: BTreeSet::new(),
            detail,
            verifier,
        });
        entry.triggers.extend(path.map(str::to_owned));
    };
    for gate in &selection.gates {
        if let Gate::Behavior { capability, .. } = gate {
            let id = capability.as_deref().unwrap_or("*");
            add(
                format!("behavior:{id}"),
                None,
                "registered model".into(),
                Some(gate.clone()),
            );
        }
    }
    for path in code {
        let effect = index.effect(path);
        for id in &effect.models {
            if planned_models.contains(id.as_str()) {
                add(format!("behavior:{id}"), Some(path), String::new(), None);
            }
        }
        for id in &effect.unmodeled {
            let detail = index
                .unmodeled(id)
                .map(|cap| format!("[{}] no runnable model: {}", cap.lifecycle, cap.reason))
                .unwrap_or_default();
            add(format!("unmodeled:{id}"), Some(path), detail, None);
        }
        for kernel in kani::affected(Some(std::slice::from_ref(path)), false) {
            let gate = Gate::Kani(kernel);
            let verifier = selection.gates.contains(&gate).then_some(gate);
            add(
                format!("kani:{}", kernel.name()),
                Some(path),
                format!("registered kernel `{}`", kernel.harness()),
                verifier,
            );
        }
    }
    obligations.into_values().collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    schema_version: u32,
    decisions: Vec<Decision>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Decision {
    obligations: Vec<String>,
    changed_scope: Vec<String>,
    authority: String,
    applicability: Applicability,
    reason: String,
    #[serde(default)]
    expected_evidence: String,
    #[serde(default)]
    evidence: Vec<Evidence>,
    #[serde(default)]
    deferred: Option<Deferred>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Applicability {
    Required,
    NotApplicable,
    Deferred,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Evidence {
    /// A gate planned in this preflight run; its own result is the evidence.
    Gate { gate: String },
    /// A retained Kani run, re-validated by a planned `kani-receipt` gate.
    KaniReceipt { kernel: String, path: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Deferred {
    owner: String,
    resume_condition: String,
}

/// One accepted decision, reduced to what the outcome needs after execution.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Answer {
    applicability: Applicability,
    obligations: Vec<String>,
    reason: String,
    /// Step names whose PASS this decision requires.
    evidence: Vec<String>,
    deferred: Option<(String, String)>,
}

/// The validated record for one run: structural errors, receipt gates to plan and answers.
#[derive(Debug, Default)]
pub struct Assessment {
    pub obligations: Vec<Obligation>,
    /// Record errors found before execution; any one fails the step.
    pub errors: Vec<String>,
    /// `kani-receipt` gates the record cites; the caller adds them to the plan.
    pub receipts: Vec<Gate>,
    answers: Vec<Answer>,
    recorded: bool,
}

impl Assessment {
    /// Validate a record (when given) against the derived obligations and the planned gates.
    pub fn new(
        root: &Path,
        obligations: Vec<Obligation>,
        changed: &[String],
        planned: &[Gate],
        record: Option<&str>,
    ) -> Self {
        let mut assessment = Self {
            obligations,
            recorded: record.is_some(),
            ..Self::default()
        };
        let Some(relative) = record else {
            return assessment;
        };
        match read_record(root, relative) {
            Ok(record) => assessment.check(root, &record, changed, planned),
            Err(error) => assessment.errors.push(error),
        }
        assessment
    }

    fn check(&mut self, root: &Path, record: &Record, changed: &[String], planned: &[Gate]) {
        if record.schema_version != 1 {
            self.errors
                .push("unsupported assurance decision schema".into());
            return;
        }
        let derived: BTreeMap<&str, &Obligation> = self
            .obligations
            .iter()
            .map(|obligation| (obligation.id.as_str(), obligation))
            .collect();
        let planned: BTreeSet<String> = planned.iter().map(Gate::name).collect();
        let changed: BTreeSet<&str> = changed.iter().map(String::as_str).collect();
        let mut answered = BTreeSet::new();
        let mut receipts = BTreeSet::new();
        let mut errors = Vec::new();
        let mut receipt_gates = Vec::new();
        let mut answers = Vec::new();
        for (position, decision) in record.decisions.iter().enumerate() {
            let label = format!("decision {}", position + 1);
            let mut error = |message: String| errors.push(format!("{label}: {message}"));
            if decision.obligations.is_empty() {
                error("names no obligation".into());
            }
            let mut kernels = BTreeSet::new();
            for id in &decision.obligations {
                let Some(obligation) = derived.get(id.as_str()) else {
                    error(format!(
                        "`{id}` is not an obligation of this change set (stale or unknown)"
                    ));
                    continue;
                };
                if !answered.insert(id.clone()) {
                    error(format!("`{id}` is answered more than once"));
                }
                if let Some(path) = obligation
                    .triggers
                    .iter()
                    .find(|path| !decision.changed_scope.contains(path))
                {
                    error(format!(
                        "changedScope does not cover `{path}`, which derives `{id}`"
                    ));
                }
                if decision.applicability == Applicability::NotApplicable
                    && id.starts_with("behavior:")
                {
                    error(format!("`{id}` is a registered model: it runs or is DEFERRED, never NOT_APPLICABLE"));
                }
                if let Some(kernel) = id.strip_prefix("kani:") {
                    kernels.insert(kernel.to_owned());
                }
            }
            if let Some(path) = decision
                .changed_scope
                .iter()
                .find(|path| !changed.contains(path.as_str()))
            {
                error(format!(
                    "changedScope names `{path}`, which is not in this change set (stale record)"
                ));
            }
            if decision.reason.trim().is_empty() {
                error("needs a reviewable reason".into());
            }
            if let Err(message) = authority(root, &decision.authority) {
                error(message);
            }
            let mut evidence = Vec::new();
            let mut evidenced = BTreeSet::new();
            for item in &decision.evidence {
                match item {
                    Evidence::Gate { gate } => {
                        if !planned.contains(gate) {
                            error(format!("evidence gate `{gate}` is not planned in this run"));
                        }
                        if let Some(kernel) = gate
                            .strip_prefix("kani (")
                            .and_then(|rest| rest.strip_suffix(')'))
                        {
                            evidenced.insert(kernel.to_owned());
                        }
                        evidence.push(gate.clone());
                    }
                    Evidence::KaniReceipt { kernel, path } => {
                        match Kernel::from_name(kernel) {
                            Some(registered) if kernels.contains(kernel) => {
                                let gate = Gate::KaniReceipt {
                                    kernel: registered,
                                    receipt: path.clone(),
                                };
                                if !receipts.insert(registered) {
                                    error(format!("kernel `{kernel}` cites more than one receipt"));
                                }
                                evidence.push(gate.name());
                                evidenced.insert(kernel.clone());
                                receipt_gates.push(gate);
                            }
                            _ => error(format!(
                                "receipt kernel `{kernel}` is not a registered kernel this decision answers"
                            )),
                        }
                    }
                }
            }
            let deferred = decision
                .deferred
                .as_ref()
                .map(|d| (d.owner.clone(), d.resume_condition.clone()));
            match decision.applicability {
                Applicability::Required => {
                    if decision.expected_evidence.trim().is_empty() {
                        error("REQUIRED needs expectedEvidence".into());
                    }
                    if evidence.is_empty() {
                        error(
                            "REQUIRED cites no gate or receipt; a written status is not evidence"
                                .into(),
                        );
                    }
                    if let Some(kernel) = kernels.iter().find(|kernel| !evidenced.contains(*kernel))
                    {
                        error(format!("REQUIRED `kani:{kernel}` needs its own `kani ({kernel})` gate or receipt"));
                    }
                    if deferred.is_some() {
                        error("REQUIRED cannot also be deferred".into());
                    }
                }
                Applicability::NotApplicable => {
                    if !evidence.is_empty() || deferred.is_some() {
                        error("NOT_APPLICABLE cites no evidence and no deferral".into());
                    }
                }
                Applicability::Deferred => match &deferred {
                    Some((owner, resume))
                        if !owner.trim().is_empty() && !resume.trim().is_empty() =>
                    {
                        if decision.expected_evidence.trim().is_empty() {
                            error("DEFERRED needs the expectedEvidence it owes".into());
                        }
                    }
                    _ => error("DEFERRED needs deferred.owner and deferred.resumeCondition".into()),
                },
            }
            answers.push(Answer {
                applicability: decision.applicability,
                obligations: decision.obligations.clone(),
                reason: decision.reason.clone(),
                evidence,
                deferred,
            });
        }
        self.errors.extend(errors);
        self.receipts.extend(receipt_gates);
        self.answers.extend(answers);
    }

    /// Print the obligations and the record check for the plan (dry run included).
    pub fn print(&self) {
        if self.obligations.is_empty() {
            println!("  assurance:     no registered obligation derived (path mapping; not proof of no semantic impact)");
        } else {
            println!("  assurance obligations:");
        }
        for obligation in &self.obligations {
            let answer = match &obligation.verifier {
                Some(gate) => format!("answered by `{}` in this run", gate.name()),
                None => "needs a decision".into(),
            };
            let from: Vec<&str> = obligation.triggers.iter().map(String::as_str).collect();
            println!("  - {}: {answer}; {}", obligation.id, obligation.detail);
            if !from.is_empty() {
                println!("      from {}", from.join(", "));
            }
        }
        for error in &self.errors {
            println!("  assurance record error: {error}");
        }
    }

    /// Record the outcome of the decisions after the gates ran.
    pub fn record(&self, report: &mut Report, skipped: bool) {
        if self.obligations.is_empty() && !self.recorded {
            report.note("assurance: no registered obligation derived from the changed paths; path mapping does not prove the absence of semantic impact");
            return;
        }
        if skipped {
            report.record(STEP, Status::NotRun, "skipped by --skip");
            report.note(format!(
                "assurance obligations left without an accepted decision: {}",
                self.ids().join(", ")
            ));
            return;
        }
        let status = |name: &str| {
            report
                .outcomes
                .iter()
                .find(|outcome| outcome.name == name)
                .map(|outcome| outcome.status)
        };
        let mut problems = self.errors.clone();
        let mut answered = BTreeSet::new();
        for answer in &self.answers {
            if answer.applicability == Applicability::Required {
                for name in &answer.evidence {
                    match status(name) {
                        Some(Status::Pass) => {}
                        found => problems.push(format!(
                            "REQUIRED evidence `{name}` is {}, not PASS",
                            found.map_or("absent".into(), |status| status.to_string())
                        )),
                    }
                }
            }
            answered.extend(answer.obligations.iter().cloned());
        }
        let mut verified = 0;
        for obligation in &self.obligations {
            if answered.contains(&obligation.id) {
                continue;
            }
            match obligation
                .verifier
                .as_ref()
                .map(|gate| (gate.name(), status(&gate.name())))
            {
                Some((_, Some(Status::Pass))) => verified += 1,
                Some((name, found)) => problems.push(format!(
                    "`{}`: its verifier `{name}` is {} and no decision answers it",
                    obligation.id,
                    found.map_or("absent".into(), |status| status.to_string())
                )),
                None => problems.push(format!("`{}` has no decision", obligation.id)),
            }
        }
        if !problems.is_empty() {
            let hint = if self.recorded {
                ""
            } else {
                "; pass --assurance-decisions FILE (xtask/README.md#assurance-decisions)"
            };
            report.record(STEP, Status::Fail, format!("{}{hint}", problems.join("; ")));
            return;
        }
        let count = |kind: Applicability| {
            self.answers
                .iter()
                .filter(|answer| answer.applicability == kind)
                .map(|answer| answer.obligations.len())
                .sum::<usize>()
        };
        report.record(
            STEP,
            Status::Pass,
            format!(
                "record valid: {verified} answered by this run's verifiers, {} REQUIRED with passing evidence, {} NOT_APPLICABLE, {} DEFERRED (pending, not PASS)",
                count(Applicability::Required),
                count(Applicability::NotApplicable),
                count(Applicability::Deferred),
            ),
        );
        for answer in &self.answers {
            match (&answer.applicability, &answer.deferred) {
                (Applicability::Deferred, Some((owner, resume))) => {
                    for id in &answer.obligations {
                        report.record(
                            format!("assurance ({id})"),
                            Status::NotRun,
                            format!("DEFERRED to {owner}; resume when {resume}"),
                        );
                    }
                }
                (Applicability::NotApplicable, _) => report.note(format!(
                    "assurance NOT_APPLICABLE ({}): {}",
                    answer.obligations.join(", "),
                    answer.reason
                )),
                _ => {}
            }
        }
    }

    fn ids(&self) -> Vec<String> {
        self.obligations
            .iter()
            .map(|obligation| obligation.id.clone())
            .collect()
    }
}

fn read_record(root: &Path, relative: &str) -> Result<Record, String> {
    let path = root.join(relative);
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|_| format!("assurance decision record `{relative}` is missing"))?;
    if !metadata.is_file() || metadata.len() > MAX_RECORD_BYTES {
        return Err("assurance decision record must be a regular file of at most 256 KiB".into());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|_| "assurance decision record is not UTF-8 text".to_owned())?;
    serde_json::from_str(&text)
        .map_err(|error| format!("malformed assurance decision record: {error}"))
}

/// An owner decision is a repository file (optionally `#anchor`) or a link into this repository.
fn authority(root: &Path, authority: &str) -> Result<(), String> {
    if let Some(rest) = authority.strip_prefix(ISSUE_AUTHORITY) {
        let kind = rest.split('/').next().unwrap_or_default();
        return if matches!(kind, "issues" | "pull" | "blob" | "tree") && rest.len() > kind.len() + 1
        {
            Ok(())
        } else {
            Err(format!(
                "authority `{authority}` is not an issue, PR or file of this repository"
            ))
        };
    }
    let file = authority.split('#').next().unwrap_or_default();
    let normalized = !file.is_empty()
        && !file.starts_with('/')
        && file.split('/').all(|part| !matches!(part, "" | "." | ".."));
    if normalized && root.join(file).is_file() {
        Ok(())
    } else {
        Err(format!(
            "authority `{authority}` is not a repository file or repository link"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preflight::{behavior_index::testing::federated, graph::testing, select};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    const PUBLICATION: &str = "services/cantos-server/src/contexts/media/domain/publication.rs";
    const OBSERVED: &str = "services/cantos-identity/src/domain/ids.rs";
    const UNMODELED: &str = "unmodeled:cantos.d.unmodeled";

    fn changed(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    fn selection(paths: &[String], index: Option<&BehaviorIndex>) -> Selection {
        select::select_indexed(Some(paths), &testing::cantos(), index)
    }

    fn obligations(paths: &[&str]) -> Vec<Obligation> {
        let paths = changed(paths);
        let index = federated();
        derive(&paths, &selection(&paths, Some(&index)), Some(&index))
    }

    fn ids(obligations: &[Obligation]) -> Vec<&str> {
        obligations.iter().map(|o| o.id.as_str()).collect()
    }

    /// A disposable checkout holding only the record and an authority file.
    struct Checkout(PathBuf);

    impl Checkout {
        fn new(record: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "xtask-assurance-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("docs")).unwrap();
            fs::write(root.join("docs/owner.md"), "Owner decision.\n").unwrap();
            fs::write(root.join("decisions.json"), record).unwrap();
            Self(root)
        }
    }

    impl Drop for Checkout {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Validate `record` for `paths`, then report the outcome given each gate's result.
    fn assess(paths: &[&str], record: Option<&str>, results: &[(&str, Status)]) -> Report {
        let paths = changed(paths);
        let index = federated();
        let selected = selection(&paths, Some(&index));
        let checkout = Checkout::new(record.unwrap_or("{}"));
        let assessment = Assessment::new(
            &checkout.0,
            derive(&paths, &selected, Some(&index)),
            &paths,
            &selected.gates,
            record.map(|_| "decisions.json"),
        );
        let mut report = Report::new("preflight");
        for (name, status) in results {
            report.record(*name, *status, "");
        }
        assessment.record(&mut report, false);
        report
    }

    fn row(report: &Report, name: &str) -> (Status, String) {
        let outcome = report
            .outcomes
            .iter()
            .find(|outcome| outcome.name == name)
            .unwrap_or_else(|| panic!("no {name} row in {report:?}"));
        (outcome.status, outcome.detail.clone())
    }

    fn record(decision: &str) -> String {
        format!(r#"{{"schemaVersion": 1, "decisions": [{decision}]}}"#)
    }

    fn decision(applicability: &str, extra: &str) -> String {
        format!(
            r#"{{"obligations": ["{UNMODELED}"], "changedScope": ["{PUBLICATION}"],
                "authority": "docs/owner.md#decision", "applicability": "{applicability}",
                "reason": "the commit pointer changed", "expectedEvidence": "server tests"{extra}}}"#
        )
    }

    fn unmodeled(applicability: &str, extra: &str) -> String {
        record(&decision(applicability, extra))
    }

    const SERVER_TESTS: &str = "rust-tests (cantos-server)";

    #[test]
    fn a_code_change_to_a_registered_model_is_answered_by_its_own_gate() {
        let found = obligations(&["services/cantos-server/src/contexts/media/two.rs"]);
        assert_eq!(ids(&found), ["behavior:cantos.b.two"]);
        assert_eq!(
            found[0].verifier.as_ref().unwrap().name(),
            "behavior (cantos.b.two)"
        );
        let paths = ["services/cantos-server/src/contexts/media/two.rs"];
        let passed = assess(&paths, None, &[("behavior (cantos.b.two)", Status::Pass)]);
        assert_eq!(row(&passed, STEP).0, Status::Pass);
        // A skipped model gate leaves the obligation open: NOT_RUN never becomes PASS.
        let skipped = assess(&paths, None, &[("behavior (cantos.b.two)", Status::NotRun)]);
        let (status, detail) = row(&skipped, STEP);
        assert_eq!(status, Status::Fail);
        assert!(detail.contains("is NOT_RUN"), "{detail}");
        assert!(!skipped.succeeded());
    }

    #[test]
    fn an_unmodeled_owner_derives_an_obligation_that_needs_a_decision() {
        let found = obligations(&[PUBLICATION]);
        assert_eq!(ids(&found), ["kani:publication-commit", UNMODELED]);
        assert!(found.iter().all(|o| o.verifier.is_none()));
        assert!(found[1].detail.contains("[draft] no runnable model"));
        // A prefix-owned path derives the same obligation.
        let prefix = obligations(&["services/cantos-server/src/contexts/listening/domain/x.rs"]);
        assert!(ids(&prefix).contains(&UNMODELED), "{prefix:?}");
        let report = assess(&[PUBLICATION], None, &[(SERVER_TESTS, Status::Pass)]);
        let (status, detail) = row(&report, STEP);
        assert_eq!(status, Status::Fail);
        assert!(
            detail.contains(&format!("`{UNMODELED}` has no decision")),
            "{detail}"
        );
        assert!(detail.contains("--assurance-decisions"), "{detail}");
    }

    #[test]
    fn documents_derive_nothing_and_force_no_verifier() {
        for path in [
            "docs/README.md",
            ".agents/skills/cantos-engineering/SKILL.md",
            "services/cantos-server/contracts/studio/publication/v1/README.md",
        ] {
            assert!(obligations(&[path]).is_empty(), "{path}");
            let report = assess(&[path], None, &[]);
            assert!(report.outcomes.is_empty(), "{path}");
            assert!(report.notes[0].contains("does not prove the absence"));
        }
    }

    #[test]
    fn unknown_impact_is_an_obligation_never_no_impact() {
        let broadened = obligations(&["Cargo.lock", "README.md"]);
        assert_eq!(ids(&broadened), [UNKNOWN]);
        assert_eq!(broadened[0].triggers, ["Cargo.lock".to_owned()].into());
        // Without a registry index, code paths cannot be cleared either.
        let paths = changed(&[PUBLICATION]);
        let unindexed = derive(&paths, &selection(&paths, None), None);
        assert_eq!(ids(&unindexed), [UNKNOWN]);
        assert!(unindexed[0].detail.contains("index unavailable"));
        let report = assess(&["Cargo.lock"], None, &[]);
        assert_eq!(row(&report, STEP).0, Status::Fail);
    }

    #[test]
    fn required_needs_planned_evidence_that_actually_passed() {
        let kani = r#"{"obligations": ["kani:publication-commit"], "changedScope": ["PATH"],
            "authority": "docs/owner.md", "applicability": "NOT_APPLICABLE",
            "reason": "comment-only change; reviewed"}"#
            .replace("PATH", PUBLICATION);
        let decisions = |evidence: &str| {
            let required = decision("REQUIRED", &format!(r#", "evidence": [{evidence}]"#));
            record(&format!("{required}, {kani}"))
        };
        let gate = format!(r#"{{"kind": "gate", "gate": "{SERVER_TESTS}"}}"#);
        let passed = assess(
            &[PUBLICATION],
            Some(&decisions(&gate)),
            &[(SERVER_TESTS, Status::Pass)],
        );
        let (status, detail) = row(&passed, STEP);
        assert_eq!(status, Status::Pass, "{detail}");
        assert!(detail.contains("1 REQUIRED with passing evidence, 1 NOT_APPLICABLE"));
        assert!(passed
            .notes
            .iter()
            .any(|note| note.contains("NOT_APPLICABLE (kani:publication-commit)")));
        for result in [Status::NotRun, Status::Fail, Status::Unsupported] {
            let report = assess(
                &[PUBLICATION],
                Some(&decisions(&gate)),
                &[(SERVER_TESTS, result)],
            );
            let (status, detail) = row(&report, STEP);
            assert_eq!(status, Status::Fail, "{result}");
            assert!(
                detail.contains(&format!("is {result}, not PASS")),
                "{detail}"
            );
        }
        let unplanned = r#"{"kind": "gate", "gate": "rust-tests (cantos-forge)"}"#;
        let report = assess(&[PUBLICATION], Some(&decisions(unplanned)), &[]);
        assert!(row(&report, STEP).1.contains("not planned in this run"));
        let report = assess(
            &[PUBLICATION],
            Some(&decisions("")),
            &[(SERVER_TESTS, Status::Pass)],
        );
        assert!(row(&report, STEP)
            .1
            .contains("a written status is not evidence"));
    }

    #[test]
    fn forged_malformed_or_incomplete_records_are_not_acceptance() {
        let gate = format!(r#", "evidence": [{{"kind": "gate", "gate": "{SERVER_TESTS}"}}]"#);
        for (label, text) in [
            (
                "self-reported status",
                unmodeled("REQUIRED", r#", "status": "PASS""#),
            ),
            (
                "unknown evidence kind",
                unmodeled(
                    "REQUIRED",
                    r#", "evidence": [{"kind": "manual", "result": "PASS"}]"#,
                ),
            ),
            ("unknown applicability", unmodeled("PASS", &gate)),
            ("not JSON", "PASS".into()),
            (
                "wrong schema",
                unmodeled("REQUIRED", &gate)
                    .replace("\"schemaVersion\": 1", "\"schemaVersion\": 2"),
            ),
        ] {
            let report = assess(&[PUBLICATION], Some(&text), &[(SERVER_TESTS, Status::Pass)]);
            let (status, detail) = row(&report, STEP);
            assert_eq!(status, Status::Fail, "{label}: {detail}");
        }
        for (label, text, message) in [
            (
                "blank reason",
                unmodeled("NOT_APPLICABLE", "").replace("the commit pointer changed", " "),
                "reviewable reason",
            ),
            (
                "missing authority",
                unmodeled("NOT_APPLICABLE", "").replace("docs/owner.md#decision", "docs/absent.md"),
                "not a repository file",
            ),
            (
                "foreign authority",
                unmodeled("NOT_APPLICABLE", "")
                    .replace("docs/owner.md#decision", "https://example.com/issues/1"),
                "not a repository file",
            ),
            (
                "evidence on NOT_APPLICABLE",
                unmodeled("NOT_APPLICABLE", &gate),
                "cites no evidence",
            ),
        ] {
            let report = assess(&[PUBLICATION], Some(&text), &[(SERVER_TESTS, Status::Pass)]);
            let (status, detail) = row(&report, STEP);
            assert_eq!(status, Status::Fail, "{label}");
            assert!(detail.contains(message), "{label}: {detail}");
        }
    }

    #[test]
    fn deferral_keeps_the_obligation_open_and_names_its_resume_condition() {
        let deferred = unmodeled(
            "DEFERRED",
            r#", "deferred": {"owner": "publication owner", "resumeCondition": "a release model exists"}"#,
        );
        let kani = deferred.replace(UNMODELED, "kani:publication-commit");
        let both = deferred.replace(
            &format!("[\"{UNMODELED}\"]"),
            &format!("[\"{UNMODELED}\", \"kani:publication-commit\"]"),
        );
        assert_ne!(kani, both);
        let report = assess(&[PUBLICATION], Some(&both), &[(SERVER_TESTS, Status::Pass)]);
        let (status, detail) = row(&report, STEP);
        assert_eq!(status, Status::Pass, "{detail}");
        assert!(detail.contains("2 DEFERRED (pending, not PASS)"));
        let (status, detail) = row(&report, &format!("assurance ({UNMODELED})"));
        assert_eq!(status, Status::NotRun);
        assert!(detail.contains("publication owner") && detail.contains("a release model exists"));
        assert!(report.render().contains("explicitly skipped (NOT_RUN)"));
        let incomplete = unmodeled(
            "DEFERRED",
            r#", "deferred": {"owner": "", "resumeCondition": "later"}"#,
        );
        let report = assess(&[PUBLICATION], Some(&incomplete), &[]);
        assert!(row(&report, STEP).1.contains("deferred.owner"));
        // A registered model is run or deferred; the record cannot waive it.
        let model = record(
            r#"{"obligations": ["behavior:cantos.b.two"], "changedScope": ["services/cantos-server/src/contexts/media/two.rs"],
                "authority": "docs/owner.md", "applicability": "NOT_APPLICABLE", "reason": "unrelated"}"#,
        );
        let report = assess(
            &["services/cantos-server/src/contexts/media/two.rs"],
            Some(&model),
            &[("behavior (cantos.b.two)", Status::Pass)],
        );
        assert!(row(&report, STEP).1.contains("never NOT_APPLICABLE"));
    }

    #[test]
    fn a_record_from_another_scope_is_stale() {
        let gate = format!(r#", "evidence": [{{"kind": "gate", "gate": "{SERVER_TESTS}"}}]"#);
        let valid = unmodeled("REQUIRED", &gate);
        // The scope grew: a second owner path now derives the same obligation.
        let grown = assess(
            &[
                PUBLICATION,
                "services/cantos-server/src/contexts/listening/domain/x.rs",
            ],
            Some(&valid),
            &[(SERVER_TESTS, Status::Pass)],
        );
        assert!(row(&grown, STEP).1.contains(
            "does not cover `services/cantos-server/src/contexts/listening/domain/x.rs`"
        ));
        // The obligation is no longer derived, and the scope names a path outside the change.
        let gone = assess(
            &["services/cantos-server/src/contexts/media/two.rs"],
            Some(&valid),
            &[],
        );
        let detail = row(&gone, STEP).1;
        assert!(
            detail.contains("not an obligation of this change set"),
            "{detail}"
        );
        assert!(detail.contains("not in this change set"), "{detail}");
    }

    #[test]
    fn a_kani_receipt_plans_its_read_only_check_and_is_evidence_only_when_it_passes() {
        let receipt = record(&format!(
            r#"{{"obligations": ["kani:publication-commit"], "changedScope": ["{PUBLICATION}"],
                "authority": "docs/owner.md", "applicability": "REQUIRED", "reason": "proof input changed",
                "expectedEvidence": "a CURRENT receipt",
                "evidence": [{{"kind": "kani-receipt", "kernel": "publication-commit", "path": "artifacts/agent-runs/kani-x/metadata.json"}}]}},
              {{"obligations": ["{UNMODELED}"], "changedScope": ["{PUBLICATION}"], "authority": "docs/owner.md",
                "applicability": "NOT_APPLICABLE", "reason": "reviewed"}}"#
        ));
        let paths = changed(&[PUBLICATION]);
        let index = federated();
        let selected = selection(&paths, Some(&index));
        let checkout = Checkout::new(&receipt);
        let assessment = Assessment::new(
            &checkout.0,
            derive(&paths, &selected, Some(&index)),
            &paths,
            &selected.gates,
            Some("decisions.json"),
        );
        assert_eq!(assessment.errors, Vec::<String>::new());
        assert_eq!(
            assessment.receipts,
            [Gate::KaniReceipt {
                kernel: Kernel::PublicationCommit,
                receipt: "artifacts/agent-runs/kani-x/metadata.json".into()
            }]
        );
        for (result, expected) in [
            (Status::Pass, Status::Pass),
            (Status::Fail, Status::Fail),
            (Status::NotRun, Status::Fail),
        ] {
            let mut report = Report::new("preflight");
            report.record("kani-receipt (publication-commit)", result, "");
            assessment.record(&mut report, false);
            assert_eq!(row(&report, STEP).0, expected, "{result}");
        }
        let wrong = receipt.replace(
            "\"kernel\": \"publication-commit\"",
            "\"kernel\": \"wav-payload\"",
        );
        let report = assess(&[PUBLICATION], Some(&wrong), &[]);
        assert!(row(&report, STEP)
            .1
            .contains("not a registered kernel this decision answers"));
    }

    #[test]
    fn an_opted_in_kani_gate_answers_its_obligation_and_skip_is_explicit() {
        let paths = changed(&[PUBLICATION]);
        let index = federated();
        let selected =
            select::select_with_kani_indexed(Some(&paths), &testing::cantos(), true, Some(&index));
        let found = derive(&paths, &selected, Some(&index));
        assert_eq!(
            found[0].verifier,
            Some(Gate::Kani(Kernel::PublicationCommit))
        );
        let assessment = Assessment::new(Path::new("."), found, &paths, &selected.gates, None);
        let mut report = Report::new("preflight");
        report.record("kani (publication-commit)", Status::Pass, "");
        assessment.record(&mut report, true);
        assert_eq!(
            row(&report, STEP),
            (Status::NotRun, "skipped by --skip".into())
        );
        assert!(report.notes[0].contains(UNMODELED));
        let observed = obligations(&[OBSERVED]);
        assert!(
            ids(&observed).contains(&"behavior:cantos.c.observed"),
            "{observed:?}"
        );
    }
}
