# Contracts

- [Script IR 0.1.0](script-ir/README.md): authoritative JSON Schema, semantic validation,
  canonical digest policy and original Vietnamese compatibility fixtures.
- [Creator HTTP API v1](http/script-api.md): request/response JSON Schema, error and retry
  contract, fixtures consumed by tests against real Axum/PostgreSQL responses.

The old [illustrative episode](examples/episode-draft.json) uses `0.1.0-draft` and remains a
historical design example. The server deliberately rejects it; no silent conversion or loss of
performance meaning is allowed. No Narrative Forge interchange is implemented.

Sources are original synthetic fixtures, not private manuscripts. Rights assertions are pending
review and grant no production/publication permission. Rust request types are validated against
these contracts; generated Leptos/Kotlin clients will be added with their actual consumers.
