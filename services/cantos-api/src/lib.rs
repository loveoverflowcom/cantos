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
