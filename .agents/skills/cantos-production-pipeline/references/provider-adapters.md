# External producer adapter guidance

> [ADR 0018](../../../../docs/decisions/0018-external-audio-production.md): all model
> and audio-generation adapters execute in an **external AI host/plugin**, never in
> `services/cantos-api`. This is a recipe for agent authors, not a Rust backend interface.

## Adapter selection

The producer chooses a speech/recording provider from its own tools and confirms language,
voice availability, emotions, timing, pronunciation, training/source rights, cost and output
format. An example Google/eSpeak/VieNeu setting in Studio is **not** evidence that Cantos
ships or supports an executable adapter.

## Model call boundaries

1. Pin exact content and instruction revision; only submit permitted text.
2. Persist a local request/operation ID and uncertain-cost state before a billable call.
3. Keep provider API tokens, model weights and raw protected assets outside Cantos.
4. Map only supported controls; never translate unknown emotions silently to neutral.
5. Store raw results with a digest, tool version and real provider receipt if available.
6. Do actual audio correction, mixing and encoding in the producer runtime.
7. Deliver only final audio and the immutable provenance/QC report to Cantos through its
   scoped private upload mechanism, once implemented.

## Reconcile before you repeat

A provider timeout does not prove it failed to synthesize or bill. The external tool must
consult provider ledgers/idempotency if supported and seek user confirmation before a
potentially duplicate paid call. Cantos has no provider ledger or synth retry loop.

## Acceptance is separate from execution

A paid/completed TTS call is **not** a checked media object and a successful object upload
is **not** a published Cantos release. Only the API's observed object checksum and
editorial/rights/QC release gates authorize distribution.

## Testing external plugins

Mock provider failures and local state loss inside the plugin. Use a test object store
and producer credentials for integration; test duplicate finalize and scope rejection
against the Cantos API when those routes exist. Do not claim provider-live results from
mocked samples.
