//! Concrete text-to-speech adapters. Each consumes frozen, catalog-admitted input and reports
//! facts in the shared receipt vocabulary; none authorizes dispatch or stores a result.
pub mod google;
pub mod local;
pub mod neural;
