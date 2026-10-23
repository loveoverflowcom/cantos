# Theatre client conformance corpora

Theatre owns these shared client semantics; they are not Server wire contracts
([ADR 0046](../../../docs/decisions/0046-service-owned-contracts.md)). Theatre Web (Rust) and
Theatre Mobile (Kotlin) execute the same literal files as test inputs; neither ships them.

| Corpus | Purpose | Consumers |
| --- | --- | --- |
| [`playback/v1`](playback/v1/vectors/README.md) — [schema](playback/v1/playback.schema.json), [vectors](playback/v1/vectors/index.json) | Mock playback vocabulary and hand-authored regression vectors for the two presentation cores | Theatre Web `core::playback` tests; Gradle `generatePlaybackVectors` |
| [`client-boundary/v1`](client-boundary/v1/README.md) | Listener boundary probes over the Server's shared error envelope | Theatre Web `live::request` tests; Gradle `generateClientBoundaryVectors` |
| [client error presentation v1](client-error-presentation-v1.md) | Default origin, error taxonomy, modal lifetime and recovery semantics | Theatre Web and Mobile presentation |

Server-owned Theatre contracts (discovery, library, identity, registration) live in
[`services/cantos-server/contracts/theatre`](../../../services/cantos-server/contracts/README.md).
