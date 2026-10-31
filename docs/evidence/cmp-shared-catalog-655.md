# CMP shared metadata and native transport boundary (#655)

Source-bound implementation/evidence record for [#655](https://github.com/loveoverflowcom/cantos/issues/655),
part of [#634](https://github.com/loveoverflowcom/cantos/issues/634). Base:
`8b76e2bfbc97465fdbfd27249dec386c11e21224` (`develop`, 31 October 2026). This record
accepts neither restricted native playback nor the whole child/epic. PR/commit and exact local
receipts are linked from the delivery and the single review ledger
[#676](https://github.com/loveoverflowcom/cantos/issues/676).

## Invariant and ownership

Only the currently admitted account and visible route may display shared metadata. An audio
association alone never dispatches private playback through an unqualified URL-only player.
The service owns Work AND Unit admission, release associations and policy decisions. The
client owns strict mapping, request/session fences and presentation; it does not compute ACLs.

`SharedCatalogPort` is separate from `PublicCatalogPort` and has no cache input. The production
Android/iOS roots borrow the existing `NativeAccountController`/`NativeAuthorizedHttpPort`
authority. The shared controller retains one visible route's RAM and bounded pages; account
incarnation, request and route identity fence replies. The root checks current admission again
before showing a projection. No private row is copied to KnownCatalog, public cache, SQLite,
Library intents, bookmarks, checkpoints or preview snapshots. All diagnostic wrappers redact
private values. Native JSON response bounds remain 64 KiB by default; shared metadata explicitly
requests up to 4 MiB, preserved through rotation and bounded by both platform transports.

The actual Android MediaPlayer, Kotlin iOS and Swift AVPlayer load edges refuse marked
session-required/shared-media locations and clear the previous item/display state. Private
Play is disabled with a localized reason for both metadata-only and audio-associated rows.
One process media owner and public playback retain their existing source and semantics.

## Accepted requirements and dependencies

ADR 0054 D05/D07/D08 and ADR 0055, plus the merged service-owned shared v1 contract, supply the
requirements. S07/S08 merges are implementation progress; their unaccepted browser, Kani,
qualification and restore evidence is not inherited. S08's active private Web source teardown
failure remains separate; this source consumes the service directly and never enables Web Play.

Behavior lookup followed the catalog, public-playback-delivery and viewer-access draft
manifests/inventories/models, and draft metadata-generation/publishing entries with `model:null`.
Classification: missing native consumer lifecycle observation scope. Server authorization/model
expectations are unchanged. No new model, promotion, Kani waiver or conformance claim is made;
controlled tests exercise the production client seam. No dependency, schema, migration, SQL,
activation flag, Studio Mobile, OS share App Link or native claim control is added. Queue order
is unchanged; S10 consumes the same contract and S12 still owns restore/activation.

## Evidence layers

The final delivery receipts under ignored `artifacts/agent-runs/655/` distinguish:

| Boundary | Evidence and limit |
| --- | --- |
| Common controller/navigation | Exact JVM assertions for guest/login return, A→B→A, route/account purge, delayed success/failure, denial, duplicate/repeated pages and cursor invalidation. These use controlled coroutine scheduling and external port doubles. |
| HTTP/Identity composition | Strict S07 metadata shapes, native Bearer and expected/actual family, closed paths, retained body bound across existing token rotation, typed status/shape/timeout refusal and transport cancellation. Identity/platform transport are deterministic doubles here; this is not a real Server/Identity journey. |
| Compose | Production Shared content and Library slot on JVM desktop Skia: disabled controls, named intents, absence of private durable controls, no false Library-empty hero and VI/EN/light/dark/large-text scenarios. JVM is not Android/iOS evidence. |
| Android | Actual debug APK build/graph/origin inspection through `theatre-host android-debug`, using the nonfunctional configured origin `https://api.example.invalid`; no install or live API call. Source/compile evidence cannot qualify playback, OS callbacks or accessibility. |
| iOS | `theatre-host ios-build` reports UNSUPPORTED: installed Kotlin/Native 2.2.0 does not satisfy the required 2.2.20. Modified Swift passes `swiftc -frontend -parse` only, not framework typecheck/link/runtime. |
| Repository/assurance | Final preflight and any failures are retained in the PR/local outcome. Stale shared component qualifications are never repaired by changing hashes. Remaining selected gates and unproven obligations retain their actual status. |

The full CMP run selected 660 shared tests (657 passed, 3 skipped) and 187 sandbox tests
(183 passed, 4 skipped), with no failures. Localization drift passed; its Python suite ran
89 tests, 87 passed and 2 skipped. Rust formatting passed. Android's final debug build and
inspection passed; iOS remains UNSUPPORTED and Swift syntax parsing passed only.

Final preflight failed at the repository gate with 65 UI qualification findings. A clean read
of the pinned main checkout reproduced 63 of them; the two additions name the new Shared
composition (Button/Surface) and the JVM test (UiRoot). No other qualification finding was
removed or changed. `impact:unknown` for the canonical localization catalog has no assurance
decision. Later aggregate gates are NOT_RUN after the repository failure; the independently
run CMP/Android/localization results above remain separate. No waiver or assurance decision
file was manufactured. Generated Web Rust changed only its catalog digest; full Web/Rust/
Turso/contracts aggregate checks did not run in this delivery.

Initial Android compilation failed because the composition helper was internal across modules;
the helper was exposed for its concrete Android consumer and the build passed. An initial full
CMP run found two missing exhaustive route cases in the sandbox; those now route unavailable
and reject private snapshot export. A newly added Library fixture also initially omitted the required neutral selected-episode
identity; that test compile error was fixed. Original failed logs are retained alongside
corrected runs.

Reproducible entrypoints (installed tools, no provisioning):

```sh
cargo xtask theatre-preview verify
cargo xtask theatre-host android-debug --api-origin https://api.example.invalid
cargo xtask theatre-host ios-build --kotlin-native-home /absolute/installed/native-2.2.20
python3 scripts/localization_catalog.py --resources --check
python3 -m unittest discover -s scripts -p 'test_localization*.py'
cargo xtask preflight --changed-only --base origin/develop
```

Use Rust 1.98.1, Gradle 8.14.3, JBR 21 and Kotlin/CMP versions from the checked build files.
Commands/logs and test XML record executed/skipped counts; an unexecuted target remains unrun.

## UI evidence and remaining admission

Actual external v3 design inputs were inspected from the linked package: Library/detail/denied
390 px captures and `HANDOFF-v3.md` from #634 comment 6520551699. They are Web design specimens,
not native screenshots or acceptance. No generated image is treated as approved design.

The native composition is Medium intensity: titles/availability lead, one quiet grouped
container, current navigation and theme. It reuses catalog `button` (Tonal/Text/Disabled with
visible reason) and `surface` (AccountGroup); no shared API, tokens, palette or fonts change.
Shared component adoption still lacks fresh two-renderer qualification. Semantic source and
JVM results do not clear that repository gate.

Opened JVM captures cover 320 dp light VI, 390 dp dark VI, 390 dp light EN at 200% text and
840 dp dark VI at 200% text. Constraint frames are 1,400 dp high; the large-text detail scrolls.
Sidecars bind PNG SHA-256, frame, theme, locale, scale and reduced-motion injection. Titles and
reasons wrap without horizontal clipping in those captures; scroll semantics keep Reload
reachable. These inspect the shared composition, not platform insets or OS text preference
behavior. There was no captured pre-change native Shared screen because none existed; no pixel
fidelity delta is claimed. No motion pacing, TalkBack or VoiceOver walkthrough ran.

Resources use the existing paired VI/EN catalog/generated Kotlin owner, reusing S08 meanings
with a native-specific transport explanation. Generated Web changes only refresh the catalog
digest; there is no Web UI or playback activation change. Content text remains opaque.

## Deferred acceptance and resume

- **Private native media is unimplemented/unqualified:** the current URL-only decoders do not
  establish Bearer on every open/seek/refresh with redirect refusal. Implement a bounded native
  data source/resource loader, preserve release/session pins and redaction, then exercise
  actual Android and iOS fetch/seek/refresh/revoke/expiry/account-switch/Back/background/late
  callbacks before enabling Play. No URL-only bearer fallback or runtime override exists.
- **iOS compile:** supply an already installed supported Kotlin/Native 2.2.20, run the command
  above, then build the actual Swift host against that framework. Parsing Swift is insufficient.
- **Device/accessibility:** named Android and iOS targets, real transport bytes, large system
  text, TalkBack/VoiceOver, background/Now Playing cleanup and process lifecycle remain NOT_RUN.
  Shared metadata common tests and Android compilation do not satisfy these rows.
- **Service gaps:** restricted cover and authenticated durable Library/restore have no safe
  serving contract here. They remain unavailable; no public substitution or offline replay.
  Internal login-return exists; OS share links and native claim are separate missing entrypoints.
- **Assurance/UI:** resolve fresh preview qualifications and the missing native lifecycle
  observation scope with evidence before acceptance. No owner is invented: unassigned —
  assignment required before enabling private playback or closing the incomplete criteria.

Admitted server response bodies may finish within their existing 60-second deadline; already
buffered bytes cannot be recalled. No remote erasure, native decoder, production activation,
real mail, restore-pair or deployed claim follows from this implementation.
