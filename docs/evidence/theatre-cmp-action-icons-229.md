# Theatre CMP action icons and native Select — #229

This is implemented diagnostic work in [PR #230](https://github.com/loveoverflowcom/cantos/pull/230),
based on `develop@e20b924f8bf5573726a87a1265b1dbdb6e27c465`. It does not qualify shared
components or accept Android/iOS behavior. [#229](https://github.com/loveoverflowcom/cantos/issues/229)
remains the acceptance owner; [#228](https://github.com/loveoverflowcom/cantos/issues/228) owns
the canonical registry. No Studio Mobile, launcher, provider, API or session change is included.

## Invariant and ownership

An action's decorative glyph must describe its intent while the existing localized label,
availability and sole callback remain authoritative. Select must retain the caller's stable
value without selecting a fallback, and disclosure, selection and pending must remain distinct.

Account/auth is Medium intensity: one primary, quiet recovery tools, existing scoped auth
shape and token pairs. Lookup reused `button` (AuthPrimary/AuthSecondary/AuthOutlined/Text),
`back-button`, `icon` and native `select`, all in the existing preview catalog. No token,
dependency, public shared API, translation mechanism or second dropdown was added.
`size.icon`, `size.control`, `space.sm` and existing theme foreground pairs own geometry/color.
Material owns the popup, scrolling, positioning and input; menu/focus state stays local.

`AuthActionIcon.kt` maps app-owned intents to canonical names. Production presentation defaults
to the existing rendering. Only `CantosDemoApp` in `:preview-sandbox` explicitly enables the
candidate. It cannot enable an identity capability or change a controller, navigation stack,
return destination, account cleanup barrier or request lifetime. The diagnostic logout uses
the existing Button icon slot; the compatibility Material row stays at the default boundary.

| Existing action | Candidate / disposition |
| --- | --- |
| Begin/Open Login | `login`; no successful-auth or verified claim |
| Create account | `person_add`; existing unavailable registration stays unavailable |
| Cancel pending login | `close`; existing explicit cancellation only |
| Restore / retry cleanup | `refresh`; no automatic retry |
| Register later / Home | `home` |
| Contextual Back | existing `arrow_back` |
| Logout | `logout` in sandbox; compatibility Material row retained pending admission |
| Open Library / Membership | existing `arrow_forward` progression |
| Settings | mapping `settings`; existing settings row retains its current Material owner |
| Theme | existing light/dark Material glyphs; no new mapping |
| Player / mini-player / episode playback | existing Material play/pause + localized labels; canonical `play_arrow`/`pause` await the same admission, no broad player migration |
| Save work / bookmark | existing star means save/bookmark, not file Save; no semantic replacement with `save` |
| Download | no existing download action found in the inspected shell/player; no invented control |

The native Select trigger now reserves a trailing 24dp disclosure slot. Long labels wrap
within the remaining space. Loading uses the shared pending cue and reason, with no disclosure
glyph; the trailing slot remains reserved. Each option reserves a trailing selected-check slot.
The popup minimum width follows the measured anchor; native viewport constraints remain in
Material. Stable-value and index overloads, selected unavailable values, missing values,
option reorder and callback ownership are unchanged. No Web shape values were copied to dp.

## Evidence and limits

The JVM Compose tests exercise the real shared Select, not a copied specimen. They assert
keyboard Enter/open, selected semantics, Escape/dismissal with trigger focus return, zero
callbacks on dismissal, `0` selection, one callback per accepted selection, loading closing
the popup, retained name/reason and no automatic reopening on recovery. Existing tests retain
reorder, missing, empty and disabled-option coverage. The auth mapping test compares the
intent table and resolves each name through the generated canonical registry.

The shared suite and Theatre `:shared`/`:preview-sandbox` suite run through the existing
`cargo xtask` routes. Their final results and commands are in PR #230 and the local handoff
under `artifacts/agent-runs/extra4-229/`. Kotlin has no configured formatter/linter; changed
Kotlin is manually reviewed, compiled and tested, not claimed linted.

Opened JVM Select captures cover selected/error, open/selected/disabled option, missing and
loading at 320/light/1×, 390/dark/2× and 840/light/1× with long Vietnamese/English text.
The initial alignment checkpoint showed the wide chevron beside centered text; the same
scenarios are rerun after the text slot fills the available width. That checkpoint is not a
develop or design baseline. Captures and semantics are in
`artifacts/ui-evidence/extra4-229/`; the handoff identifies exactly which images were opened.
Theatre captures include Login, pending, cleanup, Register and signed-in logout, both themes,
English/Vietnamese and narrow 2× actions. These are host pixels and semantics, not device or AT.

`cargo xtask ui-preview cmp run` started the actual gallery Java process. CUA inventoried
`MainKt`, but both its returned application ID and display name were rejected as `Invalid app`.
No native-window interaction is claimed. Ctrl-C stopped the owned process group; a subsequent
process inventory found no remaining gallery process. The host tests remain independent evidence.

`cargo xtask compose-preview ui-system-cmp --file
packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/preview/AndroidPreviews.kt
--composable AndroidChoicesDarkLargePreview --label extra4-229-native` reported `UNSUPPORTED`:
the official Android CLI and a ready Quail 2+ IDE were unavailable. No installation or device
provisioning was attempted. Android/iOS touch, safe area/OS Back, TalkBack/VoiceOver and system
font-scale/runtime acceptance remain `NOT_RUN`; JVM results establish none of them.

The five owner-selected Library v2 references were retried through the current supported
download helper. Network access reached Library, which still reported
`Library prepare_materialize is not available`. Outcome: no transfer, no design pixels read;
technical `BLOCKED`, not denied and not an uncertain write. No design-fidelity claim is made.

## Deferred acceptance

Complete both-renderer reference/runtime/accessibility admission before enabling the candidate
outside the sandbox. Then observe the actual auth/account flow on the named Android/iOS target,
including large VI/EN labels, pending/cancel/cleanup, focus return, touch targets and a named
TalkBack/VoiceOver walkthrough. Reuse native Select; do not migrate unrelated settings/player
controls to clear a gate. Owner: unassigned — assignment required before acceptance.

Layer gates: L1 owner/dependency review and L2 intent mapping are scoped to presentation;
L3 is JVM example/semantics/interaction evidence plus compilation; L4 deterministic identity
ports exercise the existing real app composition; L5 native/reference/AT admission remains open.
No durable model, HTTP contract, database or publication rule changed. Performance is unmeasured.
Residual risk: shared qualification is still stale/unapproved, real native window/device/AT
acceptance is missing, and the selected design could not be materialized. Keep #229 open.

The independent shared Web gallery did run in Chrome on this Mac. A light/VI Button loading
recipe retained its full name and blocked reason with a decorative pending ring. A dark/EN
enabled Button at text 200% in a 320 CSS-pixel specimen frame wrapped the long label and kept
its focus ring. Click, Space and Enter each appended one intent (three total). In the catalog,
the native action-state selector switched Button/IconButton to loading; the enabled IconButton
also produced exactly three intents for click/Space/Enter. Four captures and the corresponding
AX logs are under the same local evidence directory. The frame is not the browser viewport;
this was not browser zoom 200%, touch or screen-reader evidence. The unsupported `icon-button`
recipe was rejected to the default catalog; the actual catalog control was then used, with no
invented recipe. This bounded Web counterpart does not complete shared admission.
