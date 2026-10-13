//! The non-interactive command line, the only interface this build has.

mod args;
pub mod cli;
mod commands;
mod render;
mod reply;

#[cfg(test)]
mod tests;
