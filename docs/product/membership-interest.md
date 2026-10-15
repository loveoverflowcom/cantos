# Theatre Membership interest — bounded preview contract

[Issue 128](https://github.com/loveoverflowcom/cantos/issues/128) owns the shared Membership
form/receipt/plans semantics for Theatre Web and CMP. This document records its interaction
requirements and the explicitly permitted pure-state/preview slice. It does **not** accept a
production HTTP, persistence, pricing or entitlement contract. Those boundaries remain
`needs-decision` below. Users see **Cantos**; Studio Work/script membership is unrelated.

## Four separate facts

| Fact | Meaning and owner |
| --- | --- |
| Account registration | Identity-provider account creation; client auth owners #129/#130 and Listener authority #12. Existing login/session routes do not establish signup support. |
| Membership interest receipt | Acknowledged intent to register contact information. It grants no paid status, content access or entitlement. Only a confirmed receipt may produce the received screen. |
| Plan display | An optional, separate view opened by the listener. Opening, closing or failing to load it does not change a receipt or purchase anything. |
| Active/paid/expired entitlement | Separate product/backend authority; not implemented or simulated as a result of this form. |

## Interaction contract from issue 128

The form contains **Tên của bạn**, **Email** and **Đăng ký Membership**. There is one submit,
no stepper, mandatory review, plan/payment controls or legal checkbox/links. Independent field
errors are inline and labeled; submission focuses the first invalid field. Pending prevents a
second send; recoverable errors retain inputs. Fixture names/emails are never live prefill.

Received shows **Xem các gói Membership** and **Để sau, tiếp tục khám phá**. Account-registration
completion may also open plans directly, without submitting Membership interest. Neither account
creation nor navigation implies intent to register contact information. Provider registration
and its success confirmation stay with the auth owners; this slice adds no signup capability.

Plans use an accepted source when one exists. Until then, show **Chưa công bố** without invented
plan IDs, prices, benefits, discounts, trials, auto-renewal or provider choices. Distinguish
unpublished, loading, empty, unavailable and failed plan data independently from receipt state.
No fake details modal stands in for a production business operation.

| Plans opened from | Back destination |
| --- | --- |
| Account-registration completion | That completion screen |
| Membership received | That received screen |
| Membership introduction | Introduction |
| Account | Account |
| Deep link without context | Explicit internal account fallback |

Form Back returns to introduction and retains its draft only within the same mounted session.
Return destinations are closed internal values, never caller URLs. Back/cancel fences pending
callbacks. Logout, account switch, 401 or session mismatch must clear private projections and
drafts before another account can see them. Returning through login requires a still-valid
original intent; real auth-entry integration remains with #129/#130. Navigation emits no play,
seek, release change, checkout or subscription effect.

## Explicit preview boundary

Both renderers may demonstrate these transitions through pure, immutable presentation state and
an opt-in, visibly labeled preview. The normal capability is **unavailable**: a valid form must
explain that submission is unavailable and cannot display received. A timer, local storage,
navigation callback or a client boolean cannot confirm receipt or grant authority.

Confirmed receipt and account-registration completion are separately injected synthetic fixtures.
The fixture boundary must remain visible and must not be a live-mode fallback. A matching
operation and callback fence are required even in fixture tests. An ambiguous write outcome
retains its exact bound operation/contact intent for reconciliation or same-intent retry; editing
or minting a new operation cannot silently replace that unknown outcome. Clearing the session
invalidates old callbacks and removes private state. Preview state is memory-only: reload,
process restart, native secure storage and durable receipts are not accepted by it.

Preview validation trims both fields before binding the payload. Name accepts 1–120 Unicode
scalars and no control characters; email accepts at most 254 Unicode scalars, no control or
whitespace, exactly one `@` and nonempty sides. It requires neither a dot nor ASCII-only text.
Independent findings are ordered name then email, one per field. These bounded presentation
rules establish no deliverability, identity proof or accepted production contact policy.
An unchanged known-failure retry preserves the operation/payload with a fresh callback fence;
editing that known failure begins a new intent. An unknown outcome remains bound and uneditable.

The Web preview retains its app-owned controls. The CMP preview now composes the shared
auth/account variants and official Logo Kit through the
[six-API, two-renderer admission record](../evidence/pr137-cmp-design-v2.md), with canonical
token v0.5.0 and bundled Noto fonts. This scoped admission changes no production capability,
pricing or identity policy. Other shared APIs still need their own
[admission](../../packages/ui-system/shared/README.md#preview-registration-and-adoption);
#127 retains the broader foundation/native acceptance and #122 owns mobile release isolation.
Reuse canonical tokens, fonts and assets; do not copy prototype CSS or create another palette.

## Production decisions still required

All rows are `needs-decision`; this preview selects no production policy on their behalf.

| Boundary | Required decision / acceptance owner |
| --- | --- |
| Submitter and contact | #12/#34: signed-in versus guest admission, permitted profile source, editable contact email, contact validation and privacy/retention policy. Current session DTO has no name/email projection. |
| Authority and ownership | #12: derive actor only from current sealed Listener authority, refuse Studio/native-cookie purpose mixing, authorize each private receipt read and fence account switches. UI actor IDs/email are not authentication evidence. |
| Persistence and recovery | Membership backend owner with #12: immutable receipt shape, read-after-restart, operation/body binding, duplicate and lost-response reconciliation, retention and deletion. No route, schema or table is introduced here. |
| Pricing and benefits | #34/product owner: accepted plan source and publication/version policy before presenting commercial terms. Monthly/yearly design cards are proposals. |
| Auth return | #129/#130 with #12: verified provider signup capability, guest-to-login return, browser/native Back and current-session confirmation. No credentials/operator changes here. |
| Paid access | Product/backend owner: payment and entitlement contract outside issue 128. Receipt and plan display cannot authorize paid content. |

The [behavior catalog](../../specs/behavior/catalog.json) has no Membership model/observer.
Native typed transition tests and shared preview scenarios can establish preview behavior;
neither a model PASS nor fixture replay establishes authenticated HTTP/SQL or device acceptance.
The [evidence record](../evidence/theatre-membership-preview.md) owns actual command results,
renderer observations, remaining gates and the separate independent-review handoff.
