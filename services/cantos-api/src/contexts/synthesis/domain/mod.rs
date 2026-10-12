//! Pure synthesis rules: job states, retries, attempt facts, receipts, speech identity, clip
//! reuse and regeneration planning. No database, file, process, network or clock.
pub mod attempt;
pub mod audio;
pub mod fingerprint;
pub mod jobs;
pub mod receipt;
pub mod regeneration;
pub mod reuse;
pub mod wav_inspection;
