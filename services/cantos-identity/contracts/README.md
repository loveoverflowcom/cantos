# Cantos Identity contracts

Identity owns the canonical definition of the protocol it serves
([ADR 0046](../../../docs/decisions/0046-service-owned-contracts.md)):

| Module | Authority | Fixtures | Consumers and their bindings |
| --- | --- | --- | --- |
| [`identity/v1`](identity/v1/README.md) — private Unix-socket RPC `cantos.identity.v1` (draft) | [`identity.proto`](identity/v1/identity.proto) | [`wire-goldens.json`](identity/v1/fixtures/README.md) | Identity server binding (`src/rpc/generated.rs`); Server client binding (`services/cantos-server/src/platform/identity_auth/rpc/generated.rs`); test peers on both sides |

The bindings are generated and drift-checked by `tools/proto-codegen`; consumers never import
another service's crate. A decoded message is untrusted input: the Server validates the operation,
purpose and assurance binding (`validate_fact`) and applies its own freshness and session policy.
Identity's internal file schemas are implementation documents, not contracts:
[security store](../docs/security-store-v1.md) and [mailbox store](../docs/mailbox-store-v1.md).
