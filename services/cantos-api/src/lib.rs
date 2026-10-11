//! One modular backend; Script IR and revision decisions are independent of adapters.

pub mod adaptation;
mod diagnostics;
pub mod http;
pub mod imports;
pub mod jobs;
pub mod operator;
pub mod postgres;
pub mod production;
pub mod revisions;
pub mod script_ir;
pub mod tts;
pub mod wav_inspection;
pub mod worker;
