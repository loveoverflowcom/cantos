# Synthetic production input fixtures

`settings-request.json` binds the original `two-scenes.json` speaking characters to explicit
reference-only controls and a caller-recorded synthetic planning rate. The identifiers name no
real provider, voice or price. `rights-request.json` records a **pending** private-planning
voice assertion; it intentionally grants no eligibility. These fixtures include no credentials,
consent, legal verification, audio, provider receipt, reservation or actual charge.

The [v1 schema](../../../schema/production/v1.schema.json) owns shape. Domain and actual
HTTP/PostgreSQL tests separately own resolution, invalidation, permissions, immutable candidate
identity and restart behavior. Use actual owned script/revision/operation IDs at runtime; retain
real receipts and private claims outside Git. A successful fixture decode is not authorization
to generate audio or spend money.
