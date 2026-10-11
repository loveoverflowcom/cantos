# ADR 0009: Bounded local neural Vietnamese research samples

- Status: proposed for adoption; research implementation within user-authorized local scope
- Date: 2026-10-11
- Decision owner: Cantos maintainers
- Related issue: [#6](https://github.com/loveoverflowcom/cantos/issues/6)
- Extends: [ADR 0008](0008-local-tts-inspection.md); does not replace eSpeak behavior

## Context

The user authorized local neural-engine research, lawful model/runtime installation, bounded
synthesis, implementation and a draft PR, with no merge. The same 24 GiB ARM64 Mac also runs
MyVa performance work. Heavy execution requires a coordinated window and disk headroom;
research and implementation can proceed separately. No paid API, cloud synthesis, billing,
credential grant, new access agreement or voice cloning is authorized.

At initial inspection, only about 1.4 GiB disk was free. The parent completed cleanup and granted
an exclusive heavy-work window with about 6 GiB free. The verified preset-only Nano bundle is
281,756,486 bytes; the installed runtime and model use 555.516 MiB of allocated disk, within
the 600 MiB retained-install budget. The user authorized old-cache cleanup through
the parent coordinator; Cantos must not clean concurrently or delete evidence/models/source.

[Primary-source research](../evidence/local-neural-tts-research.md) compares current Piper voices
with VieNeu-TTS v3 Nano. Piper's engine GPL license does not clear voice weights. Its Vietnamese
models have unknown, noncommercial or unresolved training lineage. VieNeu's publisher explicitly
grants Apache-2.0 rights to weights/presets and generated-output use, but its linked restricted
dataset and undisclosed training lineage prevent this slice from claiming production clearance.
The public ungated model grant supports a bounded local research inspection without downloading
the separately restricted dataset or accepting an access agreement.

## Decision

Implement one concrete **research-only** VieNeu Nano adapter inside the existing server package.
Pin model revision `aba295eb96a6fa6003ebe417cc1f2802a7adc1dc`, six artifact sizes/hashes and
SDK `vieneu==3.8.3`. Check the SDK source/preset pack and effective runtime versions; use only
the bundled Đức Trí and Mỹ Duyên presets under explicit Cantos voice IDs. The model publisher
lists additional presets, but this adapter does not claim they are supported or tested.

Reuse `ResolvedSynthesisInput`, frozen production-document reconstruction, existing attempt/
receipt rules and the WAV inspector. Preserve Script IR, production-p1, existing catalog
version and historical eSpeak/reference entries. A concrete neural adapter/output profile
versions its own behavior. No new crate, service, generic provider framework or database schema
is justified. Catalog admission is planning information; it never grants dispatch permission.

The dispatch boundary refuses production **before** runtime/model I/O and child creation,
regardless of planning approval or caller rights assertions. A private research request binds
the exact admitted input digest/dialogue. There is no accepted-asset conversion or production
endpoint; an offline receipt audit cannot grant production or publication eligibility.

Use a separately installed local Python runtime under isolated execution with the fixed helper
embedded in the Rust binary. No runtime/model download occurs at synthesis. Validate the
effective NFC text against a bounded Vietnamese alphabet/basic-punctuation predicate and
explicit neutral/default controls. Preflight phonemes against the pinned model vocabulary;
preflight every chunk's predicted duration and reject values over the sampler's 15-second cap
before any inference, rather than inheriting its silent clamp.
Do not allow upstream dropped symbols, ignored controls, default voices or emotion/cloning tags
to become Cantos fallbacks. The fixed sampling/normalization/quantization recipe is adapter
behavior, not a user-selectable unrecorded control.

Capture complete WAV on bounded stdout rather than accepting a child-controlled output path.
Convert finite mono native 24 kHz float samples to signed 16-bit PCM with a documented pinned
quantization recipe, then validate structure/frames/checksum using a distinct neural inspection
profile. Bound input/output/stderr, process concurrency, CPU time and total wall time. Monitor
child RSS; macOS monitoring is a bounded stop policy, not an exact peak or physical-memory quota.
An inspection failure receives at most 50 ms of consecutive polling grace while deadline and
pipe checks continue; a valid observation resets that gap, and persistent failure kills/reaps.
Python dependency imports use a write-denying bootstrap audit hook whose filesystem denials
are `PermissionError`, allowing harmless import capability probes to recover. Network and child
processes remain denied; a second full guard is installed before model loading. These hooks
are not an OS sandbox for trusted native dependencies.
Feed ONNX the exact verified graph bytes. Keep runtime/model installations private and stable;
SDK version and source checks do not prevent same-owner replacement of native dependencies.

The explicit `probe-neural-tts --research-only` CLI creates a new private directory and saves
conservative intent, frozen document and purpose before invocation. Save an append-only receipt
and inspected output hash. Refuse an existing destination; never automatically retry or erase
an ambiguous attempt. Unknown charge stays unknown; local resources are measured separately.
Keep all weights, audio and run records outside Git.

## Alternatives

| Option | Benefit | Limitation | Disposition |
| --- | --- | --- | --- |
| Piper Vietnamese | Small ONNX/CPU voices, published ARM64 runtime | Unknown/noncommercial/unresolved model lineage; engine license insufficient | Production NO_GO; no download in this slice |
| VieNeu Nano presets | Small CPU-only model; explicit public weights/preset grant | Training provenance unresolved; actual Mac speed/quality requires evidence | Selected for bounded research; production disabled |
| MMS Vietnamese | Explicit Vietnamese neural checkpoint | CC BY-NC weights and larger Torch dependency footprint | Deferred; no download |
| Existing eSpeak NG | Already measured compact local path | Formant speech; listening quality unrun | Preserve unchanged |
| Generic provider framework | Future providers could share transport | No concrete commercial consumer; expands authority/recovery scope | Rejected |

## Consequences

- Product and user impact: original Vietnamese research samples can exercise a neural path;
  naturalness, pronunciation and acting require named-listener assessment.
- Compatibility and migrations: no Script IR/API/schema or production digest change; historical
  snapshots remain reconstructible. Model changes require a new explicit adapter/model identity.
- Operations, storage and production cost: private pinned runtime/model only; no cloud calls,
  credits, reservations, settlement, public delivery or listener-time inference.
- Implementation and recovery: production fails closed; scoped receipts, bounded child effects,
  conservative uncertain observations, new destination and no automatic repeat.
- Deferred questions: training/preset provenance adjudication, listener acceptance, production
  rights/authorization/immutable runtime, commercial #6 path and later durable jobs/cache.

## Validation and revisit conditions

[Implementation evidence](../evidence/local-neural-tts.md) separates exact checks and mocked
process fixtures from actual model execution. No WAV hash or successful build establishes speech
quality or rights. Revisit production only after applicable training/voice/use provenance is
resolved and the existing authorization/reservation/acceptance boundary is implemented. The
current PR remains part of #6, does not close it, starts no #7 worker work and must not merge
without the user's later instruction.
