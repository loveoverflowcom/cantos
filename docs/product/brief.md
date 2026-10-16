# Cantos — Product Brief

Current direction: [ADR 0018](../decisions/0018-external-audio-production.md) keeps synthesis
outside Cantos API; [ADR 0022](../decisions/0022-cantos-forge-producer-cli.md) records the
2026-10-16 Forge local/Functions and private Drive draft direction. Prior backend TTS
experiments are historical, not a current implementation plan.

## Identity and purpose

**Cantos — Vọng Đài** is the system of record for works and their audio editions.
Studio is a **Web-only** content management and production-control application for creators
and editors; Theatre is the listening application (Web, Android and iOS).

[Account access policy](../decisions/0034-theatre-signup-and-provisioned-studio.md): Theatre
listeners self-register; Studio has no public registration and admits only one or two securely
provisioned administrator accounts. Actual issuer capability and live acceptance are separate
from this accepted product policy.

Works are **novel**, **story** or **podcast**. A narrated novel does not require characters,
scenes or casting; a human-recorded podcast does not require an adapted Script IR.

## Product structure

- Work → ordered chapter/episode with stable identity and ownership.
- Optional source/rights references, text/notes or structured Script IR.
- Immutable content and independently fetchable versioned production instructions.
- Immutable audio renditions, with language/profile, checksum and provenance.
- Exact editorial/rights/QC decisions and immutable published release pointers.

Work/category metadata and private producer drafts are distinct from accepted audio and
published releases.

## Responsibilities

| Owner | Responsibility |
| --- | --- |
| Studio Web | Catalog/editorial UI, generation controls/configuration, status, preview and explicit review/acceptance; calls the API and, when implemented, a separately authorized Forge endpoint |
| Cantos API on VPS | Authoritative Work/Content/Brief, permissions, scoped producer admission, verified private assets, reviews, release metadata and listener contracts |
| Forge / other external producers | Story/script/audio drafts, provider/tool execution, recording/mixing/encoding, producer checks, attempts and costs; outside the API runtime |
| Theatre Web/CMP | Discover and play eligible immutable releases, with native mobile playback and later background/offline/sync acceptance |

For Forge, the owner chose local CLI plus Firebase Functions over the **same Rust application
core**, with no separate Forge GUI. Private personal Google Drive is the remote draft workspace
for applicable source/script/index/state/audio-preview artifacts; deliberate offline local work
uses filesystem. Drive success is neither API admission nor publication. Firebase Hosting serves
static Web assets; it is distinct from Forge Functions and the VPS API.

## User journeys

1. Create/import a work and organize chapters or podcast episodes and permissions.
2. Edit optional text/Script IR, then pin a content revision and production brief.
3. An external producer makes audio. The planned Forge path lets Studio request a bounded
   operation and preview its private drafts; local CLI and manual external production remain valid.
4. The owner prepares an exact scoped submission; the producer transfers bytes through the
   implemented Audio Lifecycle's bounded binary PUT, which performs admission/finalization.
5. Studio reviews verified private bytes, provenance, rights and named QC/listening findings.
6. Publish an eligible immutable release; Theatre discovery and playback consume that release.

The [general handoff proposal](../../packages/contracts/external-audio-handoff-v1.md) describes
broader ideas. Its direct-object-store prepare/finalize design is not the implemented route
contract; [Audio Lifecycle v1](../../packages/contracts/audio-lifecycle-v1.md) owns today's flow.

## MVP scope

Catalog → content → immutable instructions → external audio → review → release → Theatre
playback. Keep Script IR/casting optional and Studio Web-only. Forge may perform provider
execution outside the API; editing, publishing or listening must never implicitly start synthesis.

## Implementation status

Reviewed on 2026-10-16 at develop `25b1c701`; source presence and historical test results are
separate from current deployment acceptance.

| Capability | Current implementation / remaining boundary |
| --- | --- |
| Catalog/editorial | Work/ContentUnit CRUD, immutable content/script revisions, import/adaptation and bounded Studio consumers exist. [#66](https://github.com/loveoverflowcom/cantos/issues/66) is completed for Catalog + Studio consumer; remaining QA is tracked separately |
| Brief and private audio | Immutable Brief/read grants, scoped uploads, byte inspection, receipts, named review and candidates exist; [contract](../../packages/contracts/audio-lifecycle-v1.md) and [evidence](../evidence/audio-lifecycle.md) bound the claim |
| Private storage/artwork | Filesystem/S3 adapters, original-target reads and private Work-cover API/Studio controls exist; [storage](../architecture/object-storage.md) and [UI evidence](../evidence/studio-private-artwork-ui.md) retain provider/runtime gaps |
| Publication/listener backend | Release core, anonymous Discovery and account-owned Library/Progress exist. Origin delivery has a composition-gated core; real public delivery/activation remains unaccepted |
| Theatre clients | Web has live anonymous discovery/account reads and explicit mocks; CMP has isolated previews, production-safe shared code and an unavailable production host. Native audio/background/offline/full sync remain unfinished |
| Identity | Studio/Theatre/Admin adapters and Theatre registration state exist; real issuer/login/signup activation remains under #174/#191. Firebase email proof (#206) is not an activated replacement |
| Forge | Local artifact/register/validate/export CLI and library-only attempt/recovery core exist. Real provider execution, Functions, Drive adapter, durable recovery and API submission remain planned under #203 |
| Deployment | Manual Firebase Hosting and demo distribution tooling exist; deployed-origin/tester acceptance is separate. No Functions deployment is implied |

Use [current priorities](../work-plan/current-priorities.md) and linked issues for the next
review boundary. A merged implementation, a valid media envelope or a draft model PASS does
not establish production, listening quality or platform acceptance.
