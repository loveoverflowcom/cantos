//! One modular backend, organized by bounded context.
//!
//! - [`script_ir`] is the versioned Script IR shared kernel every context reads and writes.
//! - [`contexts`] holds the business capabilities: pure `domain` rules, `application` use cases
//!   and each context's own PostgreSQL adapter.
//! - [`interfaces`] maps HTTP onto those use cases and decides nothing itself.
//! - [`platform`] is technical plumbing with no business rule: the connection pool and
//!   migrations, session tokens, operator tools and the engine-independent incremental core.
pub mod contexts;
pub mod interfaces;
pub mod platform;
pub mod script_ir;

// Temporary compatibility entrypoints. New backend code uses the owning context paths;
// existing CLI/library consumers can migrate without changing behavior in this refactor.
#[doc(hidden)]
pub use contexts::adaptation;
#[doc(hidden)]
pub use contexts::ingestion as imports;
#[doc(hidden)]
pub use contexts::production;
#[doc(hidden)]
pub use contexts::synthesis::media::{clip_probe, wav_inspection};
#[doc(hidden)]
pub use contexts::synthesis::{
    fingerprint as speech_fingerprint, jobs, regeneration, reuse as clip_reuse, worker,
};
#[doc(hidden)]
pub use interfaces::http;
#[doc(hidden)]
pub use platform::{incremental, operator};

#[doc(hidden)]
pub mod revisions {
    pub use crate::contexts::editorial::domain::revisions::*;
    pub use crate::contexts::editorial::{permits, Access, Action};
}

#[doc(hidden)]
pub mod postgres {
    pub use crate::platform::postgres::*;

    pub mod durable {
        #[cfg(feature = "dbsp-experimental")]
        pub use crate::contexts::synthesis::summary_view;
        pub use crate::contexts::synthesis::{
            AttemptWork, DurableError, JobLease, JobSummary, ObjectProbes, PlanFacts, PlanReport,
            RecordedOutput, ReusePrepared, ReuseSelected, StepWork,
        };
    }
}

#[doc(hidden)]
pub mod tts {
    pub use crate::contexts::production::domain::catalog::{
        google as google_contract, local as local_contract, neural as neural_contract,
    };
    pub use crate::contexts::synthesis::domain::receipt::*;
    pub use crate::contexts::synthesis::media::audio;
    pub use crate::contexts::synthesis::providers::{google, local, neural};
    pub use crate::contexts::synthesis::{attempt, providers::google::http as google_http};
}
