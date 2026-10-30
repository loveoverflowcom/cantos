//! Server-side conformance of the Server's own Rust contract representation
//! (`cantos_server::contract`) against the canonical schemas and fixtures it owns under
//! `services/cantos-server/contracts/`. Clients keep separate conformance tests for their own
//! representations of the same fixtures.
mod adaptation_wire;
mod artwork_wire;
mod audio_intake_wire;
mod audio_lifecycle_wire;
mod brief_wire;
mod catalog_wire;
mod content_wire;
mod cover_disclosure_wire;
mod delegation_wire;
mod identity_auth_wire;
mod import_wire;
mod listener_library_wire;
mod metadata_publication_wire;
mod msgpack_wire;
mod production_wire;
mod protobuf_wire;
mod studio_wire;
mod theatre_discovery_wire;
