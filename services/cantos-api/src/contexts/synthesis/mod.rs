//! Synthesis: durable speech jobs over frozen production inputs, provider attempts and their
//! receipts, verified clip reuse and partial regeneration planning.
mod application;
pub mod domain;
pub mod media;
mod postgres;
pub mod providers;
pub mod worker;

pub use domain::{attempt, fingerprint, jobs, receipt, regeneration, reuse};
#[cfg(feature = "dbsp-experimental")]
pub use postgres::summary_view;
pub use postgres::{
    AttemptWork, DurableError, JobLease, JobSummary, ObjectProbes, PlanFacts, PlanReport,
    RecordedOutput, ReusePrepared, ReuseSelected, StepWork,
};
