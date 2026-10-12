//! One modular backend; Script IR and revision decisions are independent of adapters.

pub mod adaptation;
pub mod clip_probe;
pub mod clip_reuse;
mod diagnostics;
pub mod http;
pub mod imports;
pub mod incremental;
pub mod jobs;
pub mod operator;
pub mod postgres;
pub mod production;
pub mod regeneration;
pub mod revisions;
pub mod script_ir;
pub mod speech_fingerprint;
pub mod tts;
pub mod wav_inspection;
pub mod worker;
