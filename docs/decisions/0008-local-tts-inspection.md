# ADR 0008: Bounded local TTS inspection with eSpeak NG

- Status: superseded by [ADR 0018](0018-external-audio-production.md); the local eSpeak runtime it selected was removed. Kept as history
- Date: 2026-10-10
- Decision owner: Cantos maintainers; implementation choice within user-authorized local scope
- Related issue: [#6](https://github.com/loveoverflowcom/cantos/issues/6)
- Extends: [ADR 0007](0007-casting-production-inputs.md)
- Supersedes: the no-local-install boundary for this authorized inspection slice only

## Context

The user authorized local runtime/library/model installation and actual local TTS after the
initial provider-neutral investigation. Cloud/commercial calls and paid spending remain
unapproved. The Mac mini has limited remaining disk space; no heavy model environment is needed
to establish one real local speech path. Issue #6 still requires a commercial live sample and
its provider/terms/budget decision, so this slice cannot claim full issue completion.

The existing modular server owns immutable production snapshots and authorization; Script IR
owns editorial text and performance intent. The WAV inspector validates bounded bytes.
Local TTS must consume these contracts without a second rights, script or publication authority.
Source and generated speech remain outside Git. Distinguish a real local sample from synthetic
fixtures and from Vietnamese listening quality.

## Decision

Use separately installed eSpeak NG 1.52.0 for a bounded, nonprivate Vietnamese inspection run.
The supported built-in voices are exactly `vi`, `vi-vn-x-central` and `vi-vn-x-south`.
All three have installed availability and actual sample-byte evidence in the
[local inspection record](../evidence/tts-receipt-inspection.md).
This is compact formant synthesis with bundled language rules, requiring no downloaded neural
voice weights. It is a verification choice, not an expressive-quality or production-publication
decision. Official package, voice, format and license findings live in
[provider prerequisites](../evidence/tts-provider-prerequisites.md).

Keep the concrete local adapter and receipt inspector inside the existing Rust server package.
No new service/crate, generic provider framework, cloud credential store or listener-time
generation is justified. Version the supported local catalog/adapter contract explicitly.
Reject unsupported voice, model and performance controls before invoking the executable.
Bind effective text, snapshot/dialogue identity, tool/catalog/adapter version and output profile
to the attempt and resulting output hash. Reuse immutable snapshot approval and WAV inspection
rather than treating a caller receipt as a fresh authorization.

Invoke the executable with bounded stdin and private regular-file output. Append a newline to
complete UTF-8 input because this engine's bulk stdin path removes its final byte. Pin and
inspect the installed voice table; the native CLI's fallback selection is not Cantos policy.
The model ID identifies the adapter/version contract, not an immutable runtime/data pin.
This sample-only probe assumes a trusted stable installation; record post-run executable,
voice-data-tree and voice-table fingerprints as observations. Production identity/cache use
must address replacement and dependencies separately. No cache or artifact reuse exists here.
Record native PCM WAV bytes, format and digest after validation instead of asserting a format
from documentation. Do not convert or relabel the native sample rate silently.

Promote already locked transitive `libc` **0.2.190** to an exact direct Unix dependency only
for bounded local filesystem handling: opening audio nonblocking, rejecting non-regular files
and avoiding a FIFO/device read that defeats the byte bound. Keep this use at the
CLI/infrastructure boundary; pure production/domain rules remain independent of Unix and the
provider. No new runtime or architectural abstraction accompanies this dependency.

Local synthesis makes no provider billing request and has no proved external deduplication or
result-lookup contract. Do not infer universal zero cost: local resources are measured separately
and billable-provider costs remain unknown until explicit policy and evidence exist. Preserve
ambiguous attempts after dispatch; a crash, timeout or lost response cannot justify exactly-once
synthesis. This decision alone creates no durable worker/retry ledger, automatic redispatch,
reservation settlement or production deployment.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| eSpeak NG 1.52.0 built-in Vietnamese | Small separate engine; published ARM64 bottles; no neural-weight download | Robotic sound; pronunciation/performance need live evidence; GPL distribution obligations | Selected for authorized local inspection |
| Piper Vietnamese | Small neural model candidates | Voice restrictions differ; VAIS/Lessac lineage unresolved; Vivos expressly noncommercial/share-alike | Deferred pending model rights and separate quality/resource decision |
| Facebook MMS Vietnamese | Explicit Vietnamese checkpoint | CC BY-NC 4.0; Torch footprint; production rights not cleared | Deferred; unnecessary for this local verification |
| Commercial provider such as Azure Speech | Declared Vietnamese neural voices and PCM | Account/terms/rights/pricing/budget/authentication and live sample absent | Deferred until selected and authorized |
| Generic provider framework | Broad future flexibility | No commercial consumer or verified contract; expands retry/accounting scope | Rejected for this focused slice |

## Consequences

- Product and user impact: one real local Vietnamese sample can exercise inspection. This does
  not promise natural acting, creator-ready speech or Cantos Theatre publication.
- Compatibility and migrations: preserve Script IR, original scripts/rights/snapshots and
  existing receipt versions; no migration or publication-manifest change is required here.
- Operations, storage and production cost: install through Homebrew within authorized local
  scope; record package/dependency identity and disk use. Keep generated audio and private run
  artifacts outside Git.
  No cloud transmission, credentials or paid synthesis accompanies this selection.
- Implementation and recovery: enforce bounded regular-file inspection, explicit capability
  rejection, snapshot/output binding and uncertainty visibility. Synthetic tests cannot satisfy
  the live sample gate; unrun recovery or quality checks remain **NOT_RUN**.
- Deferred questions: commercial provider/cap, production identity/worker ownership, durable
  dispatch/reconciliation, broader voice/publication rights and expressive quality.

## Validation and revisit conditions

Primary sources establish declared engine/license/voice/package capabilities. The implementation
must record installation, actual local synthesis, byte validation and receipt verification,
including version/data/voice identity and private sample checksum/path. Required repository
checks and fresh review remain merge gates. Listening observations must name an evaluator;
sample peak and WAV integrity cannot establish listening quality or true peak.

GNU GPL version 3 permits running the program; output is covered only when that output itself
constitutes a covered work. Keep license/source notices and review obligations before
redistributing engine/data. The software license grants no manuscript rights or publication
approval.

Revisit before production acting, bundling binaries/data, adopting a neural model, commercial
dispatch, durable workers or publication. Issue #6 remains open until missing commercial and
complete production criteria are handled.
