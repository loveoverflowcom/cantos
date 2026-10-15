# Work-scoped delegation research evidence — 14 October 2026

## Claim and provenance

This records a **documentation-only** decision-pending slice of
[#113](https://github.com/loveoverflowcom/cantos/issues/113), not Work-delegation implementation.
[The RFC](../architecture/work-scoped-delegation-rfc.md) owns the comparison/responsibility
sketch; [the handoff](../work-plan/work-scoped-delegation-handoff.md) owns recommended next gates.

- Source: verified `origin/develop` `b6066acc659b6d4cd030badb9c1abdad08b6dcf2`.
- Environment: isolated Linux cloud worktree, Python 3.12.14, `cryptography` 50.0.0;
  repository checks use the already-installed Rust/Cargo 1.98.1 toolchain.
- Inputs: public deterministic test bytes/key seed, synthetic `.invalid` issuer/audience,
  synthetic UUIDs/actors, time classes 99/100/150/199/200. No private content or real credential.
- Disposable source SHA-256:
  `183fa799d1898e8b9c1d08ad06a86d7aa7c966954acb249e9467c6fdc50c434c`.
- Ignored local run record: `artifacts/agent-runs/issue113-session1/fixtures.py`,
  `fixtures.log`, `results.json`. The fixture is deliberately not a production source/dependency,
  repository entrypoint, supported protocol SDK or committed automated regression gate.
- Exact public source is now preserved in the [archival Markdown appendix](work-scoped-delegation-fixture.md),
  with hash-checked fresh-checkout extraction; ignored files are not the only reproduction route.

Issue #113 has no comments at the initial read. Its "no code or migration" boundary is preserved:
only Markdown is changed. Small disposable experiments satisfy the comparison step without
introducing a runnable grant service, a schema, live credentials or an accepted algorithm.

## Two representative mechanisms, shared independent policy

**A — stateful opaque mechanics.** Encode 32 public test bytes as canonical unpadded base64url;
lookup `SHA-256(test-purpose-prefix || raw_bytes)` in one in-memory grant map. Reject malformed,
short and unknown values. This is **not** the existing Cantos 64-hex wire shape and selects no
future prefix/public ID/checksum/hash construction. Deterministic bytes establish no CSPRNG claim.

**B — signed scoped mechanics.** Use the installed Ed25519 primitive to sign compact JWS-style
header/payload segments. Pin `alg=EdDSA`, a fixture-only type/key, issuer/audience/purpose;
reject duplicate JSON members, malformed encoding/signature and changed immutable binding.
The fixture compares verified claims with a fixed registered grant before applying current facts.
It is not a JWT library, RFC-complete access token, PASETO implementation, secure production
parser or distributed verifier. It does not evaluate refresh/renewal or signing-key rotation.

Both feed a separate pure illustrative policy over known delegate, active delegate/grantor,
grant/parent revocation, current owner, authoritative Work/unit/revision, selected action,
`not_before <= now < expiry`, archive state and independent domain permission. Fixture error
precedence is a test choice only; the RFC does not select production denial precedence.

### Observed exact assertions

**PASS: 64 exact-outcome assertions**, including 23 shared cases per mechanism, one synthetic
I/O-gap revoke case per mechanism, signed-transport/static-binding partitions, opaque malformed/
unknown/short partitions, proof-valid/online-revoked and purpose-separated lookup.

| Shared case partition | Exact fixture result |
| --- | --- |
| Allowed content read/write, at not-before, just before expiry | `allow` |
| Work B, other unit, changed authoritative Work ancestry | `foreign_resource` |
| Other immutable revision | `pin_mismatch` |
| Publish, membership, new-grant issuance or Admin operation | `action_denied` |
| Unknown delegate | `unknown_delegate` |
| Disabled delegate/grantor | `inactive_actor` |
| Revoked grant or parent | `revoked` |
| Grantor no longer current owner | `grantor_no_longer_owner` |
| Just before not-before or exactly at expiry | `outside_window` |
| Archived resource | `archived` |
| Independent Editorial/domain refusal | `independent_domain_denied` |
| Cookie plus delegation bearer | `mixed_credentials` |

Signed cases separately refuse tampering/truncation/invalid input, `none`, unknown key,
wrong type/issuer/audience/purpose, duplicate `sub` and signed-but-foreign scope. Signature proof
still succeeds when external revoke facts change, but the online policy returns `revoked`.
This is the crucial demonstrated distinction: **proof validity is not current authority**.

Each synthetic I/O-gap case first allows, replaces the current parent fact with revoked, then
rechecks and observes zero synthetic commits. This is `example-tested` fixture behavior, not
fault injection in Cantos, an application test, real I/O or a PostgreSQL concurrency guarantee.

**PASS: eight finite subset-oracle cases.** A narrower read-only child with interval 120–180
is accepted; restored publish, extended expiry, earlier start, changed Work/unit/revision and
an empty window are refused. This models a subset relation over fixed pins and action sets only.
It does **not** verify Biscuit/Macaroons cryptography, descendant identity, rule semantics,
real parent chains, attenuation availability or inability to reuse an original parent token.
No Quint model/property/conformance or Kani proof was run.

### Mechanics measurements, not a performance selection

Five rounds per operation, monotonic `perf_counter_ns`, repeated fixed warm input, serial local
Python execution; median/min/max per-call nanoseconds below. No concurrent clients, DB/network,
new grant map growth, production parser, storage, HTTP or end-to-end request is included.

| Operation | Calls per round | Median ns | Min–max ns |
| --- | --- | --- | --- |
| SHA-256, purpose prefix + 32 bytes | 10,000 | 325 | 319–338 |
| HMAC-SHA-256, same bytes and public fixture key | 10,000 | 1,214 | 1,155–1,252 |
| Canonical opaque decode + hash + in-memory lookup | 10,000 | 2,749 | 2,726–2,767 |
| Signed parse + Ed25519 verify + static binding | 2,000 | 147,615 | 145,399–149,386 |

The opaque encoding was **43 ASCII bytes**; the signed fixture with its particular exact
UUIDs, actors and claims was **751 ASCII bytes**. Neither includes a proposed PAT UX envelope,
HTTP framing or a production header budget. These numbers are observations of these inputs,
not universal token sizes or a Rust algorithm benchmark. HMAC/SHA ratios do not establish
cryptographic superiority, useful latency savings or a reason to reject B/C. BLAKE3 and actual
PASETO/Biscuit/Macaroons sizes/performance are **NOT_RUN**.

## Reproduction and disposal

Use the [exact Markdown appendix](work-scoped-delegation-fixture.md#hash-checked-replay-from-a-fresh-checkout)
from any fresh checkout; its stdlib extraction verifies the recorded source SHA-256 before
running in a new temporary directory. No session-local artifact is required. Replay needs
already-installed Python/`cryptography` and enabled assertions; missing prerequisites are
**NOT_RUN**. Record actual source/tool versions and all named outcomes. Never replace the
first-run timing observation with replay timings.

The second session copied the original fixture into a separate ignored run directory, verified
its unchanged source hash and independently reproduced all **64 exact outcomes + 8 fixed subset
cases**, including both zero-synthetic-commit rechecks and both exact token sizes. It then
replayed the appendix extraction from committed documentation alone, with the same stable results.
The first session's source, JSON and logs were preserved. Each run writes only adjacent results;
none reads secret files or performs network I/O.

Independent inspection also demonstrated a known non-production parser limitation: a correctly
signed JSON array payload raises `AttributeError`, rather than a typed refusal. This negative
control is not one of the passing 64 cases and does not invalidate their recorded outcomes.
It does rule out any broader hostile-parser/fail-closed claim. See the appendix's limits; no
Cantos source uses this illustrative parser. A durable regression suite belongs to a later
reviewed implementation, never to a second handwritten policy SSOT.

## Verification ledger

| Layer / claim | Status | Evidence / limits |
| --- | --- | --- |
| L1 actual boundary inventory and A/B/C/hybrid comparison | Documented | Current source read and primary-source review; not implementation acceptance |
| L2 proposed principal/policy/type barriers | Documented | Conceptual private proof/permit construction and typed refusals; no compiled or type-enforced claim |
| L3 synthetic A/B/subset experiments | PASS | 64 + 8 exact assertions; `example-tested` disposable fixtures only |
| Repository document/link/text/Python gate | PASS | `cargo xtask verify repository`; 392 Python tests, document/link validation and staged/unstaged whitespace checks; not an application test |
| Changed-only preflight | PASS | `cargo xtask preflight --changed-only`; actual selection is repository + rust-format, both PASS, no skips |
| L4 Cantos delegated application/router integration | NOT_RUN | No implementation exists in this slice |
| L5 delegated PostgreSQL locks/audit/restart | NOT_RUN | No delegation tables/transactions exist; synthetic final check is not a SQL fence |
| S3/Long Vân presigned upload/readback | NOT_RUN | No credentials, provider capability check or live request; independent design |
| Live Kanidm/owner/delegate identity and incident recovery | NOT_RUN | Existing production gates unchanged |
| Rust library compatibility, Quint and Kani | NOT_RUN | Separate candidate/accepted-semantic gates; docs-only slice selects nothing |

Existing producer read/upload, Studio/Admin and storage implementations were inspected, not
retested or changed. Older evidence records are not fresh test results for this source.
No provider/device/production acceptance, completed #113 checklist, final ADR, issue closure,
deployment or generally usable agent credentials follow from this evidence.

The actual dry run `cargo xtask preflight --changed-only --dry-run` reports six documentation
paths, no Cargo package impact and repository/rust-format selection; its execution status is
**NOT_RUN** by definition, separately from the executed PASS above. Full logs are retained in
the respective ignored session run records as `repository.log`, `preflight-plan.log` and `preflight.log`. Diagnostic
fixture tests inside the repository suite intentionally print model/conformance failure and fake
Kani output; the 392-test suite itself is OK. Those adapter negative-control messages are neither
an actual delegated behavior failure nor Quint/Kani execution evidence for this RFC.
