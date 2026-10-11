# Cantos Studio Web

`cantos-studio` is a Leptos 0.7.8 CSR/WASM consumer of the actual local Axum/PostgreSQL
revision API. Its text-first workspace has scene/act/character navigation, native dialogue,
narration, emotion, pronunciation and cue controls, and a source/validation/change inspector.
Import, proposal review and accepted-revision editing keep their state mounted when switching
workspace sections. Advanced complete JSON is optional; unsupported documents stay available
there without projection or silent field deletion. Casting and immutable production-input
review use that same authority; audio production and Cantos Theatre remain planned. Cantos
provides a data/tool pipeline: generation belongs to the user's chosen Gemini, ChatGPT or Codex
host under their control. Cantos has no AI API client or generation runtime. Fixture and live host
evidence are separately recorded in the
[adaptation evidence](../../../docs/evidence/ai-script-adaptation.md).

## Build and run

Requires Rust 1.87, `wasm32-unknown-unknown`, Trunk 0.21.14 and matching wasm-bindgen 0.2.100.
From the repository root:

```sh
rustup target add wasm32-unknown-unknown --toolchain 1.87.0
cargo xtask build cantos-studio --web   # NO_COLOR=true trunk build --locked in this directory
cargo xtask verify web                  # WASM clippy plus Trunk builds of Studio and Theatre
```

Trunk fetches its matching wasm-bindgen helper on its first build. An offline build requires
that helper on `PATH` as well as the Cargo cache; the Mac run used
`~/Library/Caches/dev.trunkrs.trunk/wasm-bindgen-0.2.100`. Generated `dist/` is ignored.
Serve the built files with the local Axum host, using the [isolated database runner](../../../services/cantos-api/README.md).
The actual walkthrough used `python3 scripts/serve_studio.py` at `http://127.0.0.1:8080`; today
`cargo xtask run cantos-api --test-cluster` starts that same primitive after
`cargo xtask verify postgres --keep`.

`Trunk.toml` also declares an optional 8081 development proxy to 8080.
`cargo xtask run cantos-studio --web` starts the API with `CANTOS_WEB_ORIGIN=http://127.0.0.1:8081`
and then `trunk serve --locked`; `cargo xtask app cantos-studio --web` starts only Trunk against an
API that is already listening. That proxy mode was not used as browser evidence.

## Interaction and ownership

Sign in on each page with an operator-issued development token. It becomes a same-origin
HttpOnly/SameSite=Strict session cookie; the form clears the token after success. Credentials
are not kept in local storage or build-time config. No fake backend or fallback dataset exists.
The visible mode label identifies the live development backend.

The pure `authoring.rs` patches known leaves of the complete JSON value, preserving stable IDs,
unknown metadata, provenance, rights and selected asset fields. It rejects duplicate keys,
unsupported versions and numbers it cannot preserve exactly; backend admission still rejects
unsupported Script IR fields. Local structural undo/redo retains 40 immutable snapshots.

The pure `editor.rs` owns immutable save snapshots, actor-bound drafts, request tickets and error
transitions. A clean read opens the stored head; a dirty read preserves the local draft. A stale save requires a read/compare and explicit
"keep text, use this head as base" action. A timeout preserves the original actor, operation
key, base and snapshot; retries reconcile that same operation before another save. Session
denial after an ambiguous attempt does not discard it. A different actor cannot retry it.
Editing during a request survives its acknowledgement.

The component obtains the concrete HTTP adapter from Studio context; URLs and wire decoding
stay in `api.rs`. The browser does not decide backend permissions or Script IR validity.
`packages/contracts/api/` is shared with Axum; no server crate enters the WASM dependency graph.

Vietnamese/English resources cover every async/error state. Semantic labels, polite status,
visible focus and keyboard-operable controls use native HTML. Guarded aria-disabled actions
remain focusable with a reason. No animation or audio is implemented. Page-return handling
reapplies controlled values if Safari restores WASM state but clears autocomplete-off fields.
Native unload confirmation protects dirty, composing, buffered and unresolved states on back,
reload or close. Explicit discard requires confirmation and cannot abandon a pending mutation.
Drafts and retry snapshots remain in session memory; confirmed reload/close discards them.

The import section selects a file and explicit format, records its source and permission
evidence, then saves original bytes and the parse outcome. The receipt ID reopens the saved
source; original download returns the byte-exact upload. Failures retain the immutable request
for a same-operation retry by the same actor. A changed selection is a new operation.
Prose labels and scene suggestions remain review findings, unknown speakers remain visible,
and publication permission stays unverified. The UI renders manuscript markup as inert text.
It never assigns voices or silently submits an imported source as an accepted script.

Adaptation tools prepare a context pinned to the imported source ID/checksum, extraction version,
target script and input revision, with a recorded source export permission claim. The host generates
under the user's control and submits a bounded structured result through the tools. Studio opens
the context or proposal ID through `GET /api/v1/adaptations/{id}/review`; it never requests AI
generation. Historical proposals remain readable through the same tagged response.

The review shows preserved source text, paged extraction and warnings, the pinned input revision,
coverage claims and unsupported pacing/prosody beside the structured proposal editor and optional advanced JSON. Caller-declared host,
provider, model, settings, usage and cost are explicitly unverified; absent values stay unknown.
A valid submitted result remains a proposal. Its creator acknowledges review of the current text
and explicitly accepts a validated immutable revision through the existing guarded acceptance path.
The exact accepted revision can be reopened read-only. Ordinary editor text stays separate.
Unresolved acceptance retains the original actor, operation ID, base and JSON snapshot for exact
retry; later typing and IME composition survive responses. A changed base requires a new context.
Rights claims do not establish legal eligibility, production approval or publication permission.

Historical model evidence can be shown for preserved run metadata. The compatibility fingerprint
field records prior evidence; it does not configure a current runtime or prove a successful run.

An uncertain mutation retains its exact actor/operation/snapshot for reconciliation. A different
actor, stale response or changed input cannot silently replace the visible proposal or accepted
receipt. Revision conflicts require a new reviewed run against the current base. The structured editor is covered by the [issue #4 evidence](../../../docs/evidence/studio-script-editor.md).

## Tokens and evidence

`packages/design-tokens/studio-tokens-0.1.0.json` is the versioned source of semantic light/dark
colors and logical dimensions. Generate CSS with `python3 scripts/studio_tokens.py`; verify
drift with `python3 scripts/studio_tokens.py --check`. System/light/dark selection changes
only this page. System fonts retain Vietnamese diacritics; no downloaded font is bundled.
CMP mapping is deferred until its real runtime exists.

[Browser observations](../../../docs/evidence/script-revision-persistence.md#browser-observations)
separate real keyboard/read/save and inspected captures from pure reducer tests and compilation.
VoiceOver, OS Telex/VNI composition, exact instrumented viewport coverage and native devices
remain unverified; no complete accessibility or full Studio acceptance claim is made.

## Validation, history and review

`POST /api/v1/validation` previews structural and semantic Script IR findings without writing
records or proving source ownership. The preview and review acknowledgement bind the exact
actor/text snapshot and become unavailable after edits; save/acceptance revalidate current access,
evidence and base inside the existing transaction. Field findings retain server paths/rule codes.

`inspector.rs` consumes the existing bounded history, exact revision and script-scoped source
APIs. Historical versions and sources remain read only. The owner review checkbox names a saved
head; the backend distinguishes owner review, editor save and reader access. An uncertain review
retains its exact operation/actor/revision and remains retryable after reauthentication. History
opens never replace the draft. Stale-write resolution explicitly keeps local text while adopting
the compared head as the next base; there is no automatic merge.

Native fields buffer typing, use browser text undo and preserve composition. Pane changes,
structural actions and structural undo wait for buffered input. Escape restores an uncommitted
field; explicit removal confirms and returns focus on cancel. Source and validation are never
inserted into spoken text. The casting section plans inputs; no voice audio preview, TTS,
mixing or listener control is provided.

## Casting and production-input review

The [production v1 contract](../../../packages/contracts/production-v1.md) provides a static reference-only
capability catalog and authenticated settings, rights, preview, freeze, exact snapshot and owner
approval routes. Studio binds existing accepted script characters to synthetic reference voice/
model controls with versioned performance/pronunciation and an explicit private-planning budget.
It displays recorded claim status/term/scope, estimate and typed blockers. Missing rates remain
unknown; a caller-recorded estimate never appears as a reservation or settled charge.

Freeze preserves the exact accepted revision, complete settings, required rights versions and
resolved inputs. A blocked candidate can be inspected, but only an eligible exact owner-reviewed
candidate can receive separate production-input approval. Historical bytes stay immutable;
accepted script/settings/required-rights edits, expiry/revocation and current permission changes
reevaluate eligibility. Browser typing alone changes no backend record. The reference catalog
always reports billable dispatch unavailable, including for an approved planning candidate.

The pure `production.rs` state and `production/browser.rs` adapter remain consumers of shared
DTOs and the existing Studio context; server modules do not enter WASM. Explicit mutations
retain their actor/operation/payload for reconciliation after an uncertain response. Native
fields and localized finding/status labels use the Studio's shared focus/theme/token behavior.
[Casting evidence](../../../docs/evidence/casting-production-inputs.md) separates reducer/build
checks from actual browser interaction and inspected viewport/zoom captures.

Recorded rights assertions are not legal verification. This interface creates no real provider
account, terms acceptance, payment, generation request or audio; no cloning/real-person voice is
implied by synthetic labels. The former local generation prerequisite is superseded.
