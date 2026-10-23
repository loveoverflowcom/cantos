# Studio provider casting evidence

Review boundary: correct the existing Studio casting form so that eSpeak NG and Google Cloud
TTS inputs can express neutral delivery at intensity 0, and controls follow the exact selected
provider/model without replacing the creator's other values. This is part of
[#6](https://github.com/loveoverflowcom/cantos/issues/6), after the accepted #5 planning slice;
it does not complete #6 or advance to #7. [Issue #5](https://github.com/loveoverflowcom/cantos/issues/5)
records acceptance through PR #24 and its local evidence. Base: `develop` at
`dc7c2de311c400a7581347a80b33d9b8b34c25a4`, branch `fix/studio-provider-casting`.
Evidence below was collected on 2026-10-11 against that base plus the working-tree change.

No new ADR is needed: provider semantics, tokens, HTTP contracts, database schema and approval
rules are unchanged. The requester explicitly selected Playwright headless for the additional
browser checks when the UI tool had no available browser. This temporary verification driver
uses an existing installation; it adds no application dependency or permanent test framework.

## Invariant and boundary

Selecting a provider changes only provider/model; unsupported voice, language, performance and
pronunciation values remain inspectable and correctable. An absent intensity override and an
explicit zero remain distinct through save/reopen. The backend alone admits settings, resolves
inherited Script IR delivery, freezes inputs and grants exact owner approval.

The product owners are [business rules 6–9](../product/business-rules.md#casting-and-performance),
the [production v1 contract](../../services/cantos-server/contracts/studio/production/v1/README.md) and
[work item 020](../work-plan/020-cast-and-generate-dialogue.md). The pure Studio
[`production.rs`](../../apps/cantos-studio/web/src/production.rs) owns exact capability lookup,
provider selection and the advisory `CastingControl` report. The
[browser shell](../../apps/cantos-studio/web/src/production/browser.rs) renders those facts and
dispatches the existing commands. Studio imports the shared wire crate, not the server crate.
Form values intentionally can represent unsupported input so that it is never silently lost;
this advisory report is not a new proof of backend eligibility.

Saved settings remain append-only versions. Frozen documents and approval receipts are not
edited. No new network request, SQL query, migration, worker, reservation, retry, speech cache,
provider call or publication effect is introduced. The existing bounded reads and DBSP deferral
remain unchanged. Types enforcing backend admission and exact snapshot scope are untouched.

## Implemented behavior and claim ledger

| Claim | Failure mode | Oracle and evidence held |
| --- | --- | --- |
| Explicit neutral/0 survives serialization distinctly from inheritance | a disabled field makes the backend-required override impossible | `explicit_neutral_at_intensity_zero_round_trips_distinct_from_inheritance`; example-tested, plus browser save/freeze/approval |
| Provider selection keeps a valid model, adopts a sole model, otherwise requires a choice | a stale model leaves the form on another provider's profile | `provider_switch_changes_only_provider_and_model_and_flags_kept_values`; example-tested and keyboard interaction-tested |
| Other creator values are kept and unsupported values are flagged | provider switching silently changes performance or voice intent | the same core example; DOM assertions on retained voice, warm/600, pitch/rate, `aria-invalid` and linked notes |
| Voice/language/emotion choices and rate/pitch limits follow the exact profile | stale limits admit or hide controls for a different Google family | `unsupported_controls_follow_the_selected_profile_bounds`; example-tested; live DOM Neural2 bounds 250–2000 / −20…20 and Chirp3 HD fixed bounds |
| Profiles without intensity control offer inheritance or explicit zero | eSpeak/Google cannot override non-neutral script delivery | `espeak_and_google_profiles_accept_only_explicit_neutral_or_inherited_delivery`; example-tested; enabled native intensity select in the browser |
| Studio examples consume the server's exact catalog | hand-authored client fixtures hide provider drift | `published_catalog_fixture_is_the_served_catalog`; example-tested equality against `production_catalog()` |
| Mixed eSpeak/Google casting cannot freeze inherited non-neutral performance but can freeze explicit neutral/0 | a UI fix masks a server rejection or changes the output contract | `mixed_espeak_and_google_casting_freezes_and_approves_only_explicit_neutral_delivery`; real HTTP/PostgreSQL integration, exact 400 `invalid_request` / `unsupported_control`, then 200 review, 201 freeze, 200 approval and dispatch false |
| Unsupported controls retain a distinct keyboard focus indicator and concise accessible names | error decoration hides focus; help is duplicated inside the name | real Tab tour, exact name/state/description and computed focus assertions; accessibility-checked within the stated browser scope |
| Rapid native select changes commit the model shown by the control | recreated option nodes reset the value before the delegated change handler reads it | fast trusted Home/ArrowDown selection followed by capability/flag assertions; interaction-tested regression |

`catalog-response.json` is generated from the implemented server catalog, including eSpeak,
VieNeu Nano research and all four Google families. SHA-256:
`a1d66647b8a2d8d668ab090af5a7c8e379aec4e34eb7aea5f04de2e5e98210de`.
The server equality test guards drift. This is an internal contract fixture, not a captured Google
response, installed runtime, account-availability assertion or legal voice grant.

## Commands and results

Commands run from the repository root unless a working directory is given. Rust uses the
repository/CI-pinned 1.87.0 toolchain. The machine's 1.96.1 Clippy has a newer lint in untouched
WAV inspection tests; the pinned gate remains the authority for this slice.

| Command | Result and boundary |
| --- | --- |
| `cargo +1.87.0 fmt --all -- --check` | PASS |
| `cargo +1.87.0 clippy --workspace --all-targets --locked -- -D warnings` | PASS; native static checks |
| `cargo +1.87.0 test --workspace --locked` | PASS: 377 passed; 55 ignored (52 PostgreSQL, three opt-in tests); includes 20 production reducer examples, four new |
| `cargo +1.87.0 clippy -p cantos-studio -p cantos-theatre --target wasm32-unknown-unknown --locked -- -D warnings` | PASS |
| `RUSTUP_TOOLCHAIN=1.87.0 NO_COLOR=true trunk build --locked` from `apps/cantos-studio/web` | PASS, Trunk 0.21.14; debug CSR WASM build |
| `python3 scripts/reference_script_ir.py` | PASS, two independent c1 goldens |
| `python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS: 103 |
| `python3 scripts/test_postgres.py` with container-backed binaries and `RUSTUP_TOOLCHAIN=1.87.0` | PASS: new disposable cluster, 12 revision + 9 import + 21 adaptation + 10 production tests (52 total), plus HTTP kill/restart journeys |
| `node target/ui-evidence/studio-provider-casting/walkthrough.cjs` | PASS: trusted keyboard input, save/freeze/approval, exact accessible names, focus and seven viewport/theme/locale/text-scale cells; zero browser page errors |
| `python3 scripts/check_repository.py`; `git diff --check` | PASS: local links, JSON syntax, text/skill checks and whitespace |

Raw output remains local under ignored `target/`: `studio-casting-workspace-tests.log`,
`studio-casting-python-tests.log`, `studio-casting-trunk-build.log`,
`studio-casting-postgres-final.log`, `revision-evidence/` and
`ui-evidence/studio-provider-casting/`. Captures, synthetic session credentials, disposable
database files and generated bundles are excluded from the commit.

The final workspace test run needed loopback socket access for the existing Google HTTP
transport fixtures. A sandboxed attempt failed at socket bind with `PermissionDenied`; the
rerun with local socket access passed. Its failed output is retained separately in
`target/studio-casting-workspace-tests-sandbox.log`. These transport fixtures use local test
servers, not a Google endpoint.

The PostgreSQL runner creates a new disposable cluster rather than reading an inherited
`DATABASE_URL`. This host has no PostgreSQL binaries on `PATH`; the already-local
`postgres:17-alpine` image supplies PostgreSQL 17.11 through `cantos-pg-tools` and three tiny
`podman exec -i` wrappers for `initdb`, `pg_ctl`, `psql`. The container shares loopback and mounts
the repository at its existing path. The final invocation was:

```sh
env PATH="/home/manhpd/Projects/cantos/target/studio-casting-pgbin:$PATH" RUSTUP_TOOLCHAIN=1.87.0 python3 scripts/test_postgres.py > target/studio-casting-postgres-final.log 2>&1
```

The final cluster was `target/revision-evidence/cluster-1791710922255687670/pgdata`; the runner
stopped it successfully without `--keep`. The earlier retained browser cluster
`cluster-1791708718901532496` was stopped with `pg_ctl -m fast -w stop`, and its Studio host was
stopped before the final run. `podman rm -f cantos-pg-tools` then removed the task-owned tools
container. The temporary launch configuration and stale legacy build output were moved under
ignored `target/studio-casting-local-only/` and excluded from the commit.

## Browser journey and UI scope

Renderer: Leptos 0.7.8 CSR, debug WASM, Google Chrome 153.0.8010.36 on Linux, headless through
Playwright 1.61.1. Backend: `python3 scripts/serve_studio.py`, port 8080, the disposable PostgreSQL
cluster retained by `scripts/test_postgres.py --keep`. Original fixture: `Ánh đèn cuối sân khấu`,
`Một lời hẹn`, with An, Minh and `Người dẫn chuyện`. Authentication and all rights/budget records
are synthetic local test data. No fake backend or provider synthesis response participates in
this browser journey; no actual provider is called.

Intent: let the creator select compatible casting while inspecting preserved intent. Primary
content is the per-character form; the immediate action is Save settings. Rights, budget and
the exact input review remain separate existing sections. Surface intensity: Medium, retaining
the existing calm Lavender Stage treatment. No token values or motion were added. Unsupported
state uses explicit text plus an outline/bar, with the existing focus token; it is not color-only.
Copy keys and notes are provided in both vi-VN and en.

The browser reopens the saved mixed lineup, switches An through a reference profile with
warm/600/pitch 3 to Google, verifies the model is cleared, and explicitly selects Neural2.
Voice/performance values remain unchanged and invalid choices have associated help. Neural2
accepts rate 1200 and its declared pitch range; selecting Chirp3 HD updates the fixed bounds.
The keyboard tour traverses provider → model → voice → language → rights → rate → pitch →
emotion → intensity, with visible, unobscured focus and controls at least 44 CSS px high.

Explicit neutral/0 is then selected with keys. With Minh on Chirp3 HD and the narrator on
eSpeak, the journey saves a new settings version, reviews eligible inputs, freezes a new
snapshot, checks the confirmation by pointer and approves by Enter. Real backend responses and
current eligibility confirm `inputs_eligible: true`, `approval_current: true` and
`billable_dispatch_available: false`. The existing owner review and six synthetic rights claims
were recorded in the earlier walkthrough; this rerun reuses them. Approval is input planning,
not provider execution authorization or legal clearance. The estimate is a synthetic USD 0.04
over 135 Unicode scalars; no reservation, usage, settlement or charge was produced.

The final run saved settings version 7, froze snapshot
`50f550c7-3156-4512-ad97-f68f80363cd6` and recorded approval
`3acd39c5-3b07-4cef-93c2-86df5bf75c64` for synthetic script
`2aaab1f6-9376-4b68-ad90-7501e26ef1d1`. The local `report.json`, trusted input event ledger
and accessibility-tree snapshot pin that run. It produced no browser page errors. The console
logged one resource 404; the log did not identify its URL, so this is not a claim of a clean
network log.

### Findings and same-scenario recheck

| Finding | Owner/rule and observation | Fix and recheck |
| --- | --- | --- |
| F1: unsupported style hid keyboard focus | UI-A11Y; focused invalid voice had the same 2 px dashed outline as unfocused invalid rate | restrict invalid outline to `:not(:focus-visible)`; same An/Chirp3 HD/1440×1000/light/vi scenario now has a distinct 3 px solid focus outline with 3 px offset |
| F2: help became part of each control name | UI-A11Y; AX showed `Giọng` followed by the entire unsupported note as the accessible name | explicit `aria-labelledby` targets only the visible label; `aria-describedby` retains the help; final exact-name assertions pass |
| F3: preserved choice was hidden in a narrow native select | UI-STATE; the long unsupported prefix truncated the actual value at 320 CSS px | the help now includes the escaped preserved value in full and wraps; final same-cell captures inspected with the full values visible |
| F4: rapid model keys left the capability unset | Web input contract; trusted change events named Neural2 while the form still flagged a missing model and disabled intensity | memoize selected value and option choices to keep native option nodes stable; the same rapid key sequence now selects the matching capability and passes the follow-on bounds/flag assertions |

Captures below are local-only under `target/ui-evidence/studio-provider-casting/`. Reviewer:
Codex. All nine final images below were opened and inspected. Labels and notes wrap without
overlap; preserved select values are readable in their help even when the native option is
truncated. Light/dark contrast and the distinct focus outline remain visually legible at the
listed sizes. This is `screenshot-inspected`, `cross-viewport-inspected` and
`cross-theme-inspected` evidence, in addition to the bounded DOM, interaction and accessibility
assertions; it is not a contrast measurement or screen-reader walkthrough.

| Capture | Viewport/theme/locale/text scale |
| --- | --- |
| `focus-after.png` | 1440×1000, light, vi-VN, 100%; matched focus scenario, compared with the original observed 2 px dashed focus |
| `context-1440-light-vi-100.png` | 1440×1000, light, vi-VN, 100%; surrounding casting page |
| `1440-light-vi-100.png`, `1440-dark-vi-100.png` | 1440×1000, both themes, vi-VN, 100% |
| `320-light-vi-100.png`, `320-dark-vi-100.png` | 320×1000, both themes, vi-VN, 100% |
| `1440-light-en-100.png`, `320-dark-en-100.png` | 1440×1000 light and 320×1000 dark, en, 100% |
| `1440-dark-vi-200-reduced.png` | 1440×1000, dark, vi-VN, root font size 200%, reduced-motion preference |

The text-scale cell sets root font size to 200%; it is not actual browser page zoom or an OS
font-scale test. The seven cells assert page/body scroll width equals the viewport. Static
captures do not establish motion quality. No changed animation exists; reduced-motion preference
is exercised for form operation only. Keyboard, pointer and ordinary text input were driven;
native Vietnamese IME, screen-reader walkthrough, touch device and mobile runtime were not run.
The screenshot set cannot establish Firefox, Safari, Android or iOS behavior.

## Residual risk and next work

New async races, provider crashes, timeouts or duplicate paid calls are not introduced or newly
exercised here; existing reducer
and PostgreSQL restart checks cover the unchanged command boundaries. No audio was rendered,
measured or listened to. Provider evidence is the static server fixture, not
`provider-live-tested`; the separate [Google live sample](google-tts-adapter.md) remains NOT_RUN.
Resolve its explicit rights/account/budget decisions before any paid call, keep #6 open and do
not start #7. Screen-reader, native IME, true page zoom and other browser/device evidence remain
follow-ups. Script IR validator/encoder mutations and the WAV reference check were not rerun:
those implementations are unchanged by this form slice. Several skills retain bootstrap-era
repository-reality notes; this slice does not
update the skill policy or treat those notes as an inventory of the implemented app.
