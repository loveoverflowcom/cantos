# Identity RPC v1 wire goldens

Owner: `services/cantos-identity` (the service that defines `cantos.identity.v1`).
[`wire-goldens.json`](wire-goldens.json) holds bytes written by an independent byte-level
encoder from the Protobuf encoding specification, never copied from prost output. Each consumer
builds the described value with its own generated binding and must encode exactly these bytes
and decode them back: Identity's server binding (`services/cantos-identity/tests/rpc_wire.rs`)
and the Server's client binding (`services/cantos-server/src/platform/identity_auth/rpc/tests.rs`).
The values are synthetic; they are not credentials, accounts or authentication evidence.
