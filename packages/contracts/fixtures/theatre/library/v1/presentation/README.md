# Opt-in Library presentation fixtures

These original synthetic fixtures cover `presentation=1` and the separate v1.1 `Presented*`
DTOs. Each page has an available-metadata and a null-metadata case. Null preserves the owned
register/audio and carries no explanation that could reveal private publication state.

The default v1 corpus in the parent directory remains unchanged for strict clients. The Kotlin
fixture generator exposes it as `libraryFixtures` and this subdirectory separately as
`libraryPresentationFixtures`, so the default corpus count is unaffected. Rust schema and DTO
conformance tests explicitly load this subdirectory. The CMP Library client requests
`presentation=1` (#297) and decodes these fixtures, but generated Kotlin types and fixtures alone
do not establish live API, Android/iOS or enriched-client acceptance.

Contract: [Library v1](../../../../../listener-library-v1.md).
