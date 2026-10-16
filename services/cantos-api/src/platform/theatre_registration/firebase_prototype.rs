//! Issue 206 candidate experiment, compiled only by tests. No HTTP route, configuration,
//! production adapter, credential setup, JWT verifier or session issuer can activate it.
//! Firebase project/API-key association is an external assumption, NOT established here.
//! The local fence is process memory: it proves neither SQL concurrency nor crash recovery.

mod core;
mod transport;
