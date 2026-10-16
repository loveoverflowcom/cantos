//! The implementations of the application's ports: the filesystem workspace, the OS random
//! source and a deterministic mock speech provider.
//!
//! Everything that touches the operating system lives here: the workspace directory, bounded
//! reads of operator-named files, local bundle writing and the OS random source. The adapter
//! refuses links and non-regular files in the workspace, never overwrites a record, and reports
//! what it found as typed corruption instead of guessing.

pub mod bundle;
pub mod fs;
pub mod ids;
pub mod mock_provider;

pub use fs::FsRuntime;

#[cfg(test)]
mod tests;
