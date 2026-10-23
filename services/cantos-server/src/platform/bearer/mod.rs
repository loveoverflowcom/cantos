//! Session lifetime/rotation policy shared by Identity-issued sessions (`identity_auth`).
//! The provider-bound Bearer family (`ca1_`/`cr1_`) and its OIDC verifier are retired.
pub mod client;
pub mod core;
