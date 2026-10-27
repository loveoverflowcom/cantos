pub(crate) mod account;
pub(crate) mod account_menu;
mod account_ui;
pub(crate) mod auth;
pub(crate) mod auth_copy;
#[cfg(feature = "preview")]
pub(crate) mod auth_preview;
pub(crate) mod common;
#[cfg(feature = "preview")]
pub(crate) mod discovery;
pub(crate) mod discovery_language;
#[cfg(feature = "preview")]
pub(crate) mod explore;
pub(crate) mod library;
pub(crate) mod live_player;
#[cfg(feature = "preview")]
pub(crate) mod player;
mod player_timeline;
pub(crate) mod public_cover;
pub(crate) mod public_home;
pub(crate) mod public_work;
#[cfg(feature = "preview")]
pub(crate) mod queue;
pub(crate) mod signup;
#[cfg(feature = "preview")]
pub(crate) mod work_detail;

pub(crate) mod error_dialog;
mod guest_identity;
