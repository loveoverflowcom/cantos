# Capability authority inventory

The [catalog](catalog.json) owns stable IDs and model lifecycle. This inventory separates
**approved requirement**, **implemented code**, **model assumption** and **needs-decision**.
A model PASS neither accepts a draft requirement nor demonstrates implementation conformance.
The [decision](../../docs/decisions/0027-quint-executable-behavior.md) adopts Quint for reviewed
executable stateful expectations while preserving Protobuf and native test boundaries.

## Module-owned inventories

Each runnable capability keeps its authority table beside its model; the registry ([catalog](catalog.json)) names the manifest that points to it.

| Stable capability ID | Lifecycle | Inventory |
| --- | --- | --- |
| `cantos.media.sample-metadata` | deprecated | [sample_metadata.md](../../services/cantos-server/behavior/media/sample_metadata.md) |
| `cantos.media.private-audio-admission` | draft | [private_audio_admission.md](../../services/cantos-server/behavior/media/private_audio_admission.md) |
| `cantos.media.public-playback-delivery` | draft | [public_playback_delivery.md](../../services/cantos-server/behavior/media/public_playback_delivery.md) |
| `cantos.media.public-cover-projection` | draft | [public_cover_projection.md](../../services/cantos-server/behavior/media/public_cover_projection.md) |
| `cantos.tooling.hosting-deploy` | draft | [hosting_deploy.md](../../xtask/behavior/hosting_deploy.md) |
| `cantos.production.forge-attempt` | draft | [forge_attempt.md](../../services/cantos-forge-functions/behavior/forge_attempt.md) |
| `cantos.presentation.locale-preference` | draft | [locale_preference.md](../../apps/cantos-theatre/behavior/locale_preference.md) |
| `cantos.presentation.discovery-retired-language` | draft | [discovery_retired_language.md](../../apps/cantos-theatre/behavior/discovery_retired_language.md) |
| `cantos.identity.bearer-lifecycle` | draft | [bearer_lifecycle.md](../../services/cantos-server/behavior/identity/bearer_lifecycle.md) |
| `cantos.identity.native-mailbox-proof` | draft | [native_mailbox_proof.md](../../services/cantos-identity/behavior/native_mailbox_proof.md) |
| `cantos.identity.native-session-admission` | draft | [native_session_admission.md](../../services/cantos-server/behavior/identity/native_session_admission.md) |

## Deferred capabilities

| Stable capability ID | Approved / implemented | Assumed / needs-decision | Why deferred |
| --- | --- | --- | --- |
| `cantos.publishing.lifecycle` | [Publication v1](../../services/cantos-server/contracts/studio/publication/v1/README.md) and [domain](../../services/cantos-server/src/contexts/media/domain/publication.rs) implement readiness, clearance/approval, immutable releases, active pointers and withdrawal; activation associates an exact release with the ACTIVE metadata generation in the same write unit and `associated_release_playable` is the live gate of an associated release ([ADR 0054](../../docs/decisions/0054-publication-generation-and-access.md) D06c, #649). | Real public delivery/provider acceptance remains separate; choose an exact candidate/pointer/observation scope before making a model authoritative. | Too many independent gate and delivery dimensions for this foundation pilot. No `.qnt` or conformance claim. |
| `cantos.publishing.metadata-generation` | [Studio metadata publication v1](../../services/cantos-server/contracts/studio/metadata-publication/v1/README.md) and the [pure core](../../services/cantos-server/src/contexts/catalog/domain/metadata_publication.rs) implement explicit-selection generations, per-Unit N+1 full copies, pointer CAS, receipts/replay, PUBLIC live gates and the D06 audio compatibility matrix and association ([ADR 0054](../../docs/decisions/0054-publication-generation-and-access.md) D01a/D02/D05/D06). | S05 (#651) routes anonymous Discovery, manifest and media admission through the exact generation and association, with consumer-owned Web/CMP readers and a Studio selection/request workflow. RESTRICTED audiences (S06) remain unimplemented. | Pure and real-file Turso tests cover the kernel; no `.qnt` or conformance claim until the observation scope spans the S06 transitions. S05 adds no model: the generation/CAS, client retry and disclosure obligations remain native tests, with S06 audience observation deferred. Studio [preview v1](../../services/cantos-server/contracts/studio/preview/v1/README.md) (D01b/D03) is a read of the Catalog head under the existing Catalog authority: it adds no state, transition or generation read, so it changes no observation. |
| `cantos.listening.progress` | [Listener Library v1](../../services/cantos-server/contracts/theatre/library/v1/README.md) and [reconcile core](../../services/cantos-server/src/contexts/listening/domain/reconcile.rs) implement exact-publication conditional updates, retained history and stale conflict classification. | Cross-client offline/native lifecycle and complete replay scope need review; UI mocks do not accept those semantics. | Keep the current kernel/native tests; establish publication/media access acceptance before cross-stack scenarios. |

Private storage target/version/hash/range policies retain their
[own authority](../../docs/architecture/object-storage.md); a successful model does not accept a
provider. Catalog, script visibility, saved items, offline synchronization, wider API/UI/CMP
scenarios and artwork admission are outside this pilot. Their existing code/contracts/tests
remain authoritative at their current boundaries until an explicitly reviewed model migration.

## Retired: `cantos.identity.theatre-registration` (#174)

The provider-only registration capability is `deprecated`, non-executable (`model: null` with
reason). [Retired owner](../../docs/architecture/theatre-identity.md) points to current Identity v1.
The stable ID, properties, runs, witnesses, controls and original `.qnt` files remain for source
traceability. [Original model](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/specs/behavior/identity/theatre_registration.qnt) and
[original inventory](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/specs/behavior/inventory.md) describe the SMTP/Kanidm/Firebase baseline,
not native signup acceptance. No native model is disabled by this retirement.

Native signup is [native-mailbox-proof](../../services/cantos-identity/behavior/native_mailbox_proof.md).
Explicit confirmation, exact immutable binding, replay/revoke and receipt invariants stay at
Identity v1; provider setup phases are no longer a current gate. Native session admission and
Bearer lifecycle remain executable with their controls unchanged.
