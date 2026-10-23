//! The Server's own Rust representation of the HTTP contracts it publishes, one module per
//! surface. The canonical JSON Schemas, `.proto` files and fixtures live in
//! `services/cantos-server/contracts/`; Studio Web, Theatre Web, Theatre Mobile and
//! `cantos-web-auth` keep their own representations of the same schemas and never import this.
pub mod admin;
pub mod auth;
pub mod codec;
pub mod routes;
pub mod studio;
pub mod theatre;
