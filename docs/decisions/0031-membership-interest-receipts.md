# ADR 0031: Membership interest authority and receipt lifecycle

- Status: proposed — no production policy or wire contract accepted
- Date: 2026-10-15
- Decision owner: product owner under [#34](https://github.com/loveoverflowcom/cantos/issues/34),
  coordinating Listener authority under [#12](https://github.com/loveoverflowcom/cantos/issues/12);
  named privacy/retention approver and implementation owner unassigned
- Related issue or PR: [#175](https://github.com/loveoverflowcom/cantos/issues/175)
- Supersedes / superseded by: N/A; extends the production questions in
  [Membership interest](../product/membership-interest.md#production-decisions-still-required)

## Context

#175 requests durable contact-interest submission and authorized reconciliation after a lost
response. The existing Web/CMP implementation is a session-only preview from #128, separate
from account registration, optional plans and paid access. Its field validation and synthetic
operation IDs are presentation rules, not an accepted production contact or receipt contract.

Inspection of `develop@e998fd974bd1a671fca3c811949597023dfbf549` and #175/#12/#34/#128/#129/#130
found no accepted submitter/contact/retention decision. #175/#12/#34/#128/#130 had no comments;
#129's closure explicitly concerns its UI milestone, with production identity acceptance still
separate. A closed UI issue or this ADR's merge does not accept the decisions below.
The [source and acceptance handoff](../work-plan/membership-interest-handoff.md) records the
concrete consumers and independent failure oracles. No production Membership endpoint, DTO,
table or persistence adapter is introduced here.

## Decision proposed for owner review

Recommend **A, signed-in submission first**, to reuse current sealed Listener authority and
avoid inventing guest proof recovery or linking in the first durable slice. This is a
recommendation, not a selection; even A cannot be implemented until its contact, duplicate,
retention/deletion and abuse policies are accepted. Product may select B or C instead, with
the additional obligations below. No guest flow is authorized by this proposal.

Under recommended A, derive ownership only from current authenticated Listener authority.
Treat entered name/email as unverified contact, never proof of identity or account ownership.
The current [session contract](../../packages/contracts/theatre-identity-v1.md) supplies no
name/email profile. Do not expand provider scopes or infer shared Web/native ownership from
matching email: [explicit cross-client identity linking](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/architecture/theatre-identity.md#decisions-and-critique-of-the-original-proposal)
remains separate. No interest submission or account signup grants media or paid access.

### Alternatives

| Option | Benefits | Costs and limitations | Disposition |
| --- | --- | --- | --- |
| A. Signed-in only | Existing Listener admission and account-bound private reconciliation; fewer new proof and recovery mechanisms | Login friction; editable contact is still unverified; needs account-switch fencing, deletion and rate limits; Web/CMP accounts are not automatically linked | Recommended, not selected |
| B. Independent guest interest | Guest Home can submit without an account | Requires private proof issuance/storage/rotation/loss recovery, anti-enumeration, abuse limits and deletion authority; operation ID alone cannot authorize reads; no automatic account attachment | Open alternative; policy and proof design required |
| C. Guest interest with explicit later linking | May preserve an intentional guest journey into an account | All B obligations plus proof of both parties, one-time atomic linking, race/replay/conflicting-account recovery and deletion after linking | Open alternative; no linking policy accepted |

### Policy acceptance worksheet

All rows remain unresolved. Record named approver, selected semantics, exact bounds and source
revision together; choosing A/B/C alone is insufficient. The handoff's scenario matrix is the
review checklist, not an accepted wire schema.

| Boundary | Required owner decision |
| --- | --- |
| Submitter and read authority | Select A/B/C; define current/revoked/expired identity or guest-proof admission, private receipt access and account-switch behavior |
| Contact purpose and collection | State intended use, permitted fields/profile source, editable email and verification requirements; approve privacy notice and how it fits the existing no-checkbox/no-legal-links preview flow before any production collection |
| Canonicalization and bounds | Select Unicode/whitespace/control handling, scalar and byte limits, email case/normalization rules and deliverability scope; preview's 120/254-scalar checks are candidates only |
| Intent and duplicate scope | Define canonical immutable body/version binding, operation uniqueness scope and retention; distinguish replay of one operation from a different operation expressing the same current interest; select actor/contact/campaign uniqueness without leaking another person's existence |
| Recovery and edits | Define authoritative reconciliation, in-flight/absent/unknown distinctions and safe same-intent retry; decide when a definitely failed intent may be edited and replaced |
| Retention and deletion | Select separate finite bounds for contact, receipt, deduplication material, abuse data, logs and backups; name deletion authority, completion/recovery bounds and backup-restoration treatment |
| Erasure versus replay | Decide whether minimal non-contact tombstones remain, for how long and with which reply; explain how late retries cannot recreate deleted contact during the accepted window; an email digest can still expose low-entropy PII |
| Abuse and cost | Select request/response byte limits, field limits, per-principal/proof/network/global limits, concurrency and storage caps, trusted proxy policy, rejection/retry semantics and deletion-abuse protection; no arbitrary rate or CAPTCHA provider selected |
| Versioned consumer contract | Agree schema version, operation/receipt identities, canonical body binding, exact errors/status mappings, no-store policy and shared old/new Web/CMP fixtures before DTOs or SQL |

## Consequences

- Product: production submission remains unavailable. Only server acknowledgement or authorized
  reconciliation may produce **Đã đăng ký**; previews remain explicitly synthetic.
- Compatibility and migrations: none in this proposal. After acceptance, add versioned contracts
  and additive migrations with rollout/rollback evidence; preserve existing Library receipt
  semantics rather than silently reusing its operation namespace.
- Operations and storage: no new service, dependency, provider call, deployment or contact store.
  Retention/deletion is a prerequisite for collection, not a later cleanup task. Immutable
  operation binding must not be mistaken for permission to retain PII forever.
- Independent implementation: Web preview contact-bearing `Debug` values can be redacted under
  #175's explicit no-PII-diagnostics requirement without selecting any production policy. This
  protects formatting of those values only; it does not establish a logging pipeline audit,
  secure storage, erasure, durable receipt or live acceptance.
- Deferred questions: every worksheet row above, including whether revoked/deleted are observable
  lifecycle outcomes. Optional [plan publication](../product/membership-plan-publication.md)
  under #176, payments and entitlement remain independent.

## Validation and revisit conditions

Before production code, the named product/privacy/Listener owners must accept the applicable
worksheet, then Web/CMP reviewers accept the versioned contract and fixtures. Follow the
[handoff sequence and matrix](../work-plan/membership-interest-handoff.md#delivery-and-acceptance)
for pure-core, mocked, real SQL/HTTP and separate client/device evidence. A documentation gate,
redaction regression or mocked response does not accept durable receipt behavior.
Revisit A if guest conversion becomes an accepted requirement, if cross-client identity linking
changes, or if deletion/retention makes the selected replay guarantee impossible. Do not activate
production collection while any applicable policy or required acceptance remains unresolved.
