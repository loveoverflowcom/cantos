//! The pure Forge core: immutable values, decisions and typed refusals.
//!
//! Nothing here performs I/O, reads a clock, draws randomness, starts a process, parses CLI
//! arguments or depends on a serialization framework; `tests/architecture.rs` checks this.
//! The shell supplies facts (bytes, identities, recorded history) and executes effects.

pub mod artifact;
pub mod attempt;
pub mod audio_profile;
pub mod checks;
pub mod digest;
pub mod evaluation;
pub mod generation;
pub mod handoff;
pub mod identity;
pub mod label;
pub mod lineage;
pub mod ogg_opus;
pub mod workspace_path;
