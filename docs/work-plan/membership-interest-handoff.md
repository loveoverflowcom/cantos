# Membership interest: decision and acceptance handoff

## Outcome and boundary

[Issue #175](https://github.com/loveoverflowcom/cantos/issues/175) needs a durable, private
interest receipt, with one immutable operation/body association surviving retries and restart.
This handoff prepares that review without selecting an unaccepted product rule. The
[proposed ADR 0031](../decisions/0031-membership-interest-receipts.md) owns alternatives and the
policy acceptance worksheet. All production policy rows remain unresolved.

The independently specified code boundary is redacting diagnostic formatting of the existing
Web preview's name/email values, including nested attempts and state. It adds no production
intent/receipt types, API, migration, storage, auth integration, guest linking or UI behavior.
It does not complete #175. In particular, no persisted intent or live server acknowledgement
is inferred from a successful preview test.

## Inspected baseline and immediate consumers

Source baseline: `develop@e998fd974bd1a671fca3c811949597023dfbf549`.
Read #175, #12, #34, #128, #129, #130 and their comments. #175/#12/#34/#128/#130 had no
comments accepting production Membership policy. The
[#129 closure](https://github.com/loveoverflowcom/cantos/issues/129#issuecomment-6190096650)
explicitly accepts a UI milestone, not full production identity activation. #176 remains an
independent plan-source decision. GitHub owns execution status; this file owns sequencing.

| Consumer / boundary | Actual source | Consequence for later implementation |
| --- | --- | --- |
| Web preview | [membership core](../../apps/cantos-theatre/web/src/core/membership.rs), [shared vector runner](../../apps/cantos-theatre/web/src/core/membership_vectors.rs), [preview shell](../../apps/cantos-theatre/web/src/membership_preview.rs) | In-memory `u64` operations and injected results are preview facts; do not copy them into a durable wire contract |
| CMP preview | [MembershipInterest.kt](../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/domain/membership/MembershipInterest.kt), [vector runner](../../apps/cantos-theatre/mobile/shared/src/commonTest/kotlin/com/cantos/theatre/domain/membership/MembershipVectorsTest.kt) | Contact diagnostic formatting is already redacted; shared preview vectors establish no backend policy or runtime receipt |
| Live Web account | [account owner](../../apps/cantos-theatre/web/src/application/account.rs), [live transport](../../apps/cantos-theatre/web/src/live/account.rs) | Current account/session and projection epochs own private-response fencing; no Membership submit adapter is implemented |
| Listener admission | [sealed principal](../../services/cantos-api/src/contexts/listening/auth.rs), [Theatre identity contract](../../packages/contracts/theatre-identity-v1.md) | Derive actor from current admitted authority; session IDs/actor IDs in UI are fencing/display facts, not authentication; no trusted contact projection exists |
| Theatre middleware | [BFF admission](../../services/cantos-api/src/interfaces/http/theatre_auth.rs), [origin/cache boundary](../../services/cantos-api/src/interfaces/http/mod.rs) | Current account path and bearer-mutation classifications are specific to existing resources; a future Membership route needs explicit purpose, session-fence, origin and response-cache coverage |
| Existing Library receipts | [Library HTTP](../../services/cantos-api/src/interfaces/http/theatre_library.rs), [contract](../../packages/contracts/listener-library-v1.md) | Reuse understood identity/transaction patterns only after review; this receipt is not implicitly a new Library operation or permission |

The [current preview contract](../product/membership-interest.md) remains authoritative for
presentation. Its trim/120/254-scalar validation does not settle server email canonicalization,
duplicate scope or privacy policy. Source-side `Debug` redaction prevents accidental contact
exposure through those formatters; deliberate formatting of raw field values is still outside
that protection. No real diagnostic leak or production logging test is claimed.

## Candidate failure and recovery matrix

These are acceptance obligations for a later selected contract, not DTO fields, HTTP status
codes, a database design or executed tests. Use synthetic contact only, such as `An Nguyễn`
and `an@example.invalid`. Let A/B mean distinct admitted principals, X/Y different operations,
and P/Q different canonical contact intents under the future accepted policy.

| Scenario | Required observation / policy to settle | Independent oracle and layer |
| --- | --- | --- |
| Submit X/P, then replay X/P | Return the same authoritative receipt/result without a second application | Pure classification; real row/count/identity comparison after replay and restart |
| Concurrent X/P on two connections | One durable intent/receipt association; neither response invents a receipt | Real PostgreSQL transaction race and final persisted result |
| Reuse X with Q or incompatible intent version | Typed conflict and unchanged X/P; define canonical binding before computing it | Independent expected binding vectors; real no-row-change assertion |
| Submit Y/P when X/P is already current | Distinguish `duplicate-current` from same-operation replay; select uniqueness scope and which per-intent outcome Y records | Pure duplicate matrix, then concurrent SQL including two accounts; no disclosure of another account's contact |
| Commit X/P, drop response, restart | Remain unknown until authorized reconciliation or same-X/P retry confirms the persisted result | Fault injection after commit/before response, host and PostgreSQL restart, compare exact receipt |
| Reconcile while original request is still in flight | A momentary missing row is not proof of definite failure; keep X/P bound and retry safely | Pause before commit, read from a second connection, resume write; no new Y/Q while X is unresolved |
| Reject before commit or storage is unavailable | Typed definite rejection versus unknown outcome; no fake success; define which failures leave a durable operation binding | Transaction rollback/fault injection with unchanged-state and exact response assertions |
| Edit after known failure versus unknown | Unchanged known-failure retry keeps X/P; permitted edit starts a new intent only after definite failure; unknown stays bound | Shared Web/CMP accepted vectors for fields, operation and callback fence |
| A submits, B guesses X or presents A's display/session ID | No PII/receipt access; response must not enumerate another person's contact; IDs are not proof | Real HTTP cross-account/read/retry tests with valid B credentials and unauthenticated requests |
| Expired/revoked/mixed credentials, CSRF or missing session fence | Deny without effects; unknown authority remains unavailable; cookie/native/Studio purposes stay separate | Real HTTP admission/Origin/no-store/session-header tests and unchanged rows |
| Logout/switch while response or reconciliation is pending | Clear private projections; late A result cannot show received or submit A's intent as B | Web and CMP owner tests plus browser/native lifecycle observations |
| Delete/expire contact, then replay X/P or submit concurrent Y/P | Apply accepted retention/tombstone policy; no accidental resurrection during guaranteed window; define post-window behavior | Real deletion/write races, restart and restoration tests; inspect contact, receipt, dedup, abuse and backup obligations separately |
| Guest proof loss/expiry/replay, if B/C selected | Operation ID/email never substitute for proof; bound recovery/deletion authority must be accepted | Exact-proof allow/deny/recovery fixtures and real HTTP tests; for C, atomic one-time linking under races |
| Invalid, oversized or abusive input | Bounded server checks and exact typed errors; do not echo contact into diagnostics; select byte/rate/concurrency limits first | Pure boundary vectors, bounded HTTP body/concurrency tests and captured diagnostic canaries |
| Account creation, plans navigation or receipt display | No implicit interest submission, email verification, payment, entitlement or media grant | Assert no contact write on unrelated flows; inspect effects and real database deltas |

## Delivery and acceptance

1. **Policy review:** named #34 product/privacy owner and #12 Listener owner complete ADR0031's
   worksheet, including exact finite retention/deletion and abuse bounds. Record acceptance
   explicitly; merging the proposal does not select A or accept the worksheet.
2. **Contract and pure core:** backend and Web/CMP reviewers agree versioned intent/receipt,
   canonical binding, duplicate semantics, exact outcome/error mapping and old/unknown-version
   fixtures. Keep pending/unknown as honest client knowledge, not invented server success.
   Decide whether revoked/deleted are supported before adding variants. Implement/test the
   pure rules before adapters; do not create a speculative service/crate or provider dependency.
3. **Persistence and HTTP:** extend the existing modular monolith with reviewed additive
   migration/rollback, transactions, deletion and current private admission. Inventory receipt
   lookup/dedup read paths under the
   [read-performance workflow](../../.agents/skills/cantos-engineering/references/postgresql-read-performance.md).
   Establish real SQL concurrency, crash/restart/unknown-response and HTTP privacy evidence
   before calling it durable; a fake store cannot establish those semantics.
4. **Client adoption:** Web and CMP consume the same accepted fixtures, with an explicit
   durable same-intent retry/reconciliation policy and account-fenced projection cleanup.
   Show **Đã đăng ký** only after authoritative acknowledgement. Observe separate Web and
   Android/iOS lifecycle/accessibility behavior; no timer or local mock substitutes for receipt.

### Evidence ledger and deferred gates

| Claim / layer | Evidence scope for this boundary | Deferred acceptance and next owner |
| --- | --- | --- |
| L1 ownership and policy gate | `documented`: inspected sources, proposed ADR and matrix; no accepted production policy | Named #34 privacy/product and #12 Listener approvers required |
| L2/L3 preview diagnostic privacy | New deterministic redaction regressions for draft/payload and nested state/attempt formatting; actual output belongs in the PR | Does not audit raw-field logging, durable storage or all diagnostic sinks |
| L2 production intent/receipt and errors | BLOCKED / NOT_RUN; contract decisions unresolved | Backend and Web/CMP reviewers, unassigned; accept worksheet/fixtures, then `cargo xtask verify rust` and `cargo xtask verify contracts` |
| L4 production orchestration / mocked integration | NOT_RUN; no submit/reconcile adapter added | Backend owner, unassigned; after accepted core, exercise exact timeout/retry/error paths via the affected Rust suites; report named doubles |
| L5 real PostgreSQL and HTTP | BLOCKED / NOT_RUN; no production contract/migration/route | Backend/identity owners, unassigned; after implementation, `cargo xtask verify postgres` on a disposable cluster; all applicable matrix races/restart/denial cases required |
| L5 Web/CMP live and native/provider acceptance | BLOCKED / NOT_RUN; no receipt adapter or accepted contact collection | #129/#130 with #12; `cargo xtask verify web`, `cargo xtask theatre-demo verify`, then named real browser/device/provider observations for the implemented flows; builds/JVM tests are insufficient |

Handoff runs `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask preflight --changed-only`
on the final diff and records exact selected gates/output in the PR. There is no new fixture
corpus, Quint model, DB/API runtime result or live contact collection in this boundary.
No provider, checkout, payment, entitlement, Studio Mobile, audio/publication or storage service
change is needed. Remaining production blockers are owner policy/contract acceptance, followed
by the independently evidenced implementation stages above.
