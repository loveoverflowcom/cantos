//! Opt-in Theatre registration. Email proof authorizes creating one new listener identity;
//! it never authenticates a session, resets an existing person, or admits Studio access.
pub mod application;
pub mod config;
pub mod domain;
pub mod kanidm;
pub mod mail;
pub mod mail_config;
pub mod postgres;
pub mod runtime;
