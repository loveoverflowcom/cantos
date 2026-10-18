# Opt-in Library presentation fixtures

These original synthetic fixtures cover `presentation=1` and the separate v1.1 `Presented*`
DTOs. Each page has an available-metadata and a null-metadata case. Null preserves the owned
register/audio and carries no explanation that could reveal private publication state.

The default v1 corpus in the parent directory remains unchanged for existing strict clients.
Its Kotlin fixture generator intentionally reads only that directory's JSON files. Rust schema
and DTO conformance tests explicitly load this subdirectory. Current clients do not request
the capability; generated Kotlin types alone do not establish enriched-client acceptance.

Contract: [Library v1](../../../../../listener-library-v1.md).
