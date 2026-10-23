# Host-driven script adaptation evidence

## Scope and invariant

Issue [#3](https://github.com/loveoverflowcom/cantos/issues/3),
[PR #20](https://github.com/loveoverflowcom/cantos/pull/20), branch
`feat/ai-script-adaptation`, base `adfa82ce72a57d513f55c89ba7a58883af652cb2`.
The final scope follows the user's 2026-10-10 decision: their Gemini/ChatGPT/Codex host generates
content and calls tools; Cantos supplies the data pipeline. Cantos performs no inference.
The former local-model gate is superseded, not passed. Real model quality is **NOT_RUN** and
nonblocking for this confirmed pipeline scope. Full Studio scene/character authoring remains #4.

**Invariant:** an authenticated caller can export a frozen source/revision context and submit
untrusted output; only explicit, validated, authorized editorial acceptance creates a new
immutable revision. Original source bytes, extraction and previously accepted revisions survive
invalid output, duplicate delivery, stale inputs, cancellation and process/database restart.

**Owner/boundary:** the pure adaptation kernel prepares bounded context and admits the closed
proposal into the actual Script IR reader. The existing Axum/PostgreSQL monolith owns rights
assertions, current authorization, append-only records and the shared revision transaction.
Leptos Studio owns source comparison, local JSON editing and explicit acceptance. The thin
[CLI/HTTP tool contract](../../services/cantos-server/contracts/studio/adaptation/v1/README.md) adds no inference service, MCP runtime,
provider SDK or persistent credentials. Its four tools exclude acceptance.

## Implemented and checked claims

| Claim | Oracle and evidence level | Limit |
| --- | --- | --- |
| Context binds source ID/checksum, extractor, base revision, prompt/contract and digest | Pure/wire tests plus real PG tampering and stale-base cases; example-tested, integration-tested | c1 context is frozen; current source is never substituted |
| Caller input cannot manufacture trusted source/generation/rights bindings | Closed proposal, actual Script IR validator and edited-envelope checks; type-enforced, example-tested | Citation coverage does not prove faithful meaning |
| Unknown speakers/facts and extraction warnings stay visible | Original Vietnamese fixture, exact raw null plus unresolved character/finding oracles; differentially-tested | IR requires a character reference; the placeholder is explicitly unresolved |
| Invalid submission remains recoverable; valid proposal is immutable | Exact invalid/valid receipts, corrected operation, concurrent/replayed delivery and row-count SQL oracle; integration-tested | First valid proposal settles a run; correction after that needs a new context |
| No tool call accepts a revision or invokes AI | Actual CLI journey, separate explicit accept and zero attempt-count SQL oracle; integration-tested | User-controlled hosts own generation/cost/retry outside Cantos |
| Authorization stays current, including after locks | Isolation, revocation, anonymous/hostile HTTP cases; integration-tested | Development identity/session boundary, not production adoption |
| Acceptance shares revision CAS/idempotency and atomic rollback | Existing save helper, injected insertion/backend-death failures and competing head cases; fault-injected, integration-tested | No exactly-once external generation claim |
| Settled facts survive restart without rewrite | Actual host kill, fresh PG restart, migration replay, exact context/receipt/export/source comparisons; fault-injected, differentially-tested | Disposable local service and database |
| Metadata does not imply observed generation or billing | Declared caller metadata and unknown cost/usage UI/DTO tests; example-tested, interaction-tested | Supplied metadata is unverified; missing values are unknown |
| Historical a1 records remain readable | Independent top-level a1 fixture plus current nested DTO serialization, read/replay tests and unchanged nested-layout source review; integration-tested | Not an archived-byte golden oracle for every historical nested DTO |

Additive migration 0005 distinguishes c1 from historical a1, preserves existing tables and adds
append-only submission receipts with an owner/run composite FK. Retired start/provider endpoints
fail closed (503/null); historical reads, acceptance and ambiguity reconciliation remain.
The local inference adapter, dependency, worker dispatch and configuration are removed.

## Final local execution

Executed on 2026-10-10 on Apple M4 Mac mini, 24 GiB RAM, macOS 15.6 (24G84), PostgreSQL
17.4 Homebrew. Only repository-authored synthetic Vietnamese fixtures were submitted.
Native tests exclude the ignored real-PG cases, which were separately executed below.

| Command/check | Result | Evidence |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | `caller-checks/fmt-final.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | `caller-checks/native-clippy-final.log` |
| `cargo test --workspace --locked` | PASS: 119 native tests | `caller-checks/native-tests-final.log` |
| `python3 scripts/reference_script_ir.py` | PASS | `caller-checks/reference.log` |
| `TMPDIR=/private/tmp python3 scripts/mutate_script_ir.py` | PASS: 9 killed, 0 survived | `caller-checks/mutations.log`; bounded mutation set |
| `python3 scripts/test_postgres.py --keep` | PASS: 12 revision + 9 import + 19 adaptation | `caller-checks/postgres-run.log`; new disposable cluster, never inherited DB URL |
| `cargo clippy -p cantos-studio --target wasm32-unknown-unknown --locked -- -D warnings` | PASS | `caller-checks/wasm-clippy.log`; executed with `--offline` |
| `NO_COLOR=true trunk build --locked` in `apps/web` | PASS | `caller-checks/trunk.log` |
| `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS: 68 tests | `caller-checks/python-tests.log` |
| `python3 scripts/check_repository.py` and `git diff --check` | PASS | `caller-checks/repository-check.log`, `caller-checks/diff-check.log` |
| Current Safari review/edit/accept/reopen, focus, widths and scaling | PASS within observations below | Native inputs, AX state, inspected captures and independent API comparison |
| Real AI generation/editorial quality | NOT_RUN | No model invoked; nonblocking for chosen pipeline scope |
| Audio, mobile, production deployment/identity/TLS | NOT_RUN | Outside #3 scope |

The first fresh-PG iteration failed an obsolete index-name oracle: the new owner/ID unique index
was correctly used. The second passed all 40 Rust PG cases but failed a runner oracle expecting a
nullable IR speaker; the actual IR requires an explicit unresolved character. Both assertions
were corrected against observed plans/data without weakening query budgets or unknown-speaker
requirements. A third, entirely new cluster passed the complete runner. Earlier logs are retained
as `postgres-run-iteration-1.log` and `postgres-run-iteration-2.log`; their clusters were stopped.

All named local artifacts are ignored under `target/adaptation-evidence/` unless stated otherwise.
GitHub CI/merge evidence is recorded against exact SHAs on PR #20 after push; earlier head CI is
not substituted for the final source. No additional CI workflow or Studio WASM CI job was added.

## Actual tool transport and restart oracle

The runner launches the real executable host with the application DB role, invokes
`scripts/cantos_adaptation_tool.py` as subprocesses, and crosses the authenticated loopback HTTP
socket. It exports context, submits invalid output and a valid corrected proposal, reads review,
then separately accepts it. Raw HTTP malformed/oversized probes reject before any writes.
After abrupt host death and PostgreSQL restart, all four CLI tools and acceptance replay return
exact records; independent SQL row counts remain unchanged and generation attempts remain zero.

`external-tool-restart.json` records run `0c982dd4-2d4d-41cb-9549-e3b1bd7df430`, source
`src_6adb2010e7d84ba8a3f595c3ad919398`, script `bfc235e3-6ce3-461c-bdd3-b9fa103fa1b1` revision 1,
source SHA-256 `746accd319e847e4f9b05401794bc6fd0b3bd479e0815f197d4882806fe1335a` and
`cantos-import-1`. The fixture separates narrator/dialogue, emotion, ambience/music/SFX and pacing
review notes. Provider/model/configuration/usage/cost are null, rather than fabricated model proof.

CLI hostile-origin/path/action, ambiguous JSON, UTF-8/surrogate, nonfinite-number and resource
bounds are example-tested. Lost, malformed, oversized or truncated write responses are classified
as unknown outcome with exact-operation retry guidance; no automatic retries or new IDs occur.
Actual process/storage restart is integration-tested; socket timeout/response corruption remain
injected transport doubles, not a live network-chaos or model-timeout claim.

## Current Studio observation

Native Safari was driven against the built Leptos app on task-owned loopback port 38080 and the
successful disposable cluster. Run `27d87812-5270-409a-8158-76c392bcc817` used the same original
source and a new target script. Observations were interaction-tested, accessibility-checked,
screenshot-inspected, cross-viewport-inspected and cross-theme-inspected:

- Original Vietnamese text/extraction warnings, unresolved speaker, coverage warning and pacing
  note remained visible beside the proposal. Metadata explicitly says caller-declared/unverified;
  missing provider/model/token/cost remained unknown.
- A corrupted character ID was rejected with `/characters/1/id · id_format`, keeping the draft.
  Correcting the JSON reset review acknowledgement. Explicit acknowledgement and acceptance
  saved revision 1; “reopen exact accepted revision” showed the immutable export.
- Independent API oracle (`caller-checks/studio-before.json`, `studio-after.json`) confirmed only
  the intended scene title changed to “Bến sông — đã đối chiếu”; frozen source and original
  proposal remained exact. Accepted digest:
  `sir-e1:sha256:2eed50d480633b3c5b17cf4330453eed476c03871b399703784b113584c07bf5`.
- Inspected browser-window captures at 639×852 and 1324×968 pixels: stacked narrow flow and wide
  source/proposal columns, wrapped identifiers/JSON and readable Vietnamese. Browser frame is
  included in these dimensions; they are not claimed as DOM viewport measurements.
- Safari Page Menu confirmed 200% zoom at narrow width; text/identifiers wrapped and the accepted
  revision section stayed readable. Option-Tab advanced from reopen action to the script-ID
  field with a visible focus ring. Light/system and dark comparisons were inspected.

These captures were inspected in the execution thread, not saved as repository image files.
No VoiceOver walkthrough, mobile/device run, motion audit or full authoring acceptance is claimed.
Vietnamese/system theme and 100% zoom were restored; only the task window was closed. The
original user Safari window and port 8080 were preserved. Task-owned host and PG cluster stopped.

## Read inventory and measured baseline

| Read/consumer | Tables/query and write dependencies | Consistency |
| --- | --- | --- |
| Context and review GET, Studio open/refresh | Actor/session, owner+ID run, preserved source, base revision, proposal/submission/acceptance | Fresh authorization and coherent transaction; bounded point reads |
| Context/submission/acceptance exact replay | Owner+operation unique indexes, frozen request/digest and receipt | Exact replay or conflict; no newest-input substitution |
| Legacy review | a1 decoder plus same source/revision/proposal facts | Historical frozen metadata, current access |

`point-lookups.json` holds PostgreSQL version, plans and 30 sorted samples: 1,000 synthetic
proposal runs, 118-byte source, 2,827-byte IR, five warm-ups, concurrency 1. Measured warm in-process
HTTP Router p50 **3.564 ms**, p95 **3.728 ms**, p99 **3.746 ms**; declared p95 budget ≤250 ms passed.
UUID+actor lookup used `adaptation_run_owner_identity`; owner+operation replay used the existing
unique index. Plans report buffer hits and no sort/spill. The same response was checked each read.

Simpler indexed PostgreSQL point reads meet this bounded target. No aggregate/materialized cache
consumer justifies DBSP: **defer**, with a future measured aggregate workload and real-engine
comparison prerequisite. No speedup, cold-cache/socket latency, high concurrency, production
throughput, CPU/RAM profile, independent index-storage/write-amplification or DBSP experiment is
claimed. Seeding 1,000 context/proposal writes took 10.092 s; this is fixture setup, not a general
write-latency benchmark. All writes stay on the authoritative transaction path.

## Residual risks and handoff

Real host-generated Vietnamese quality, hallucination/omission rates, cross-document continuity,
model capability and provider billing are unmeasured. Caller provenance is a declaration, not
attestation. The CLI is actually callable by an authorized terminal-capable AI host; no standalone
ChatGPT/Gemini web account connector or production remote tool registration was configured.
Production identity/TLS and legal rights eligibility still require their own adoption work.

Continue with #4 using this context/read/submit/review contract and common accept/save path for
full scene/dialogue/character editing, clear diffs and editor acceptance. Keep uncertainty and
provenance visible. Casting, TTS, publication, audio quality and native playback remain later work.

The initial local-adapter snapshot at `e9ae8a9f1391d40985cbf010cb46a1caee3435b1` is historical:
[old execution record](https://github.com/loveoverflowcom/cantos/blob/e9ae8a9f1391d40985cbf010cb46a1caee3435b1/docs/evidence/ai-script-adaptation.md),
[old review](https://github.com/loveoverflowcom/cantos/blob/e9ae8a9f1391d40985cbf010cb46a1caee3435b1/docs/evidence/ai-script-adaptation-review.md).
Its fixture passes do not establish the current tool path or real model inference. See the
[current independent review](ai-script-adaptation-review.md) for the confirmed-scope ledger.
