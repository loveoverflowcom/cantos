//! Media's Turso persistence adapters: the public-listener reader and discovery index. The
//! Studio intake, lifecycle and publication write adapters still carry PostgreSQL SQL in
//! `postgres` until their own versions.
pub(super) mod listener;

#[cfg(test)]
mod tests;
