# Studio ActorBadge review — issue 742

Source-bound review for [#742](https://github.com/loveoverflowcom/cantos/issues/742).
Fetched base: `origin/develop@9a4687b89b29d6e42fea7b02dbc2df5f71110718`, 6 November 2026.
The PR binds the final checked source. Reviewer: Codex, one automated reviewer.

## Current consumer and decision

The issue's `b6facf5d` snapshot predates the preferences-footer change in
[#582 / PR #749](https://github.com/loveoverflowcom/cantos/pull/749).
Commit `b008432f78514d67982938564de39ad7b8478cd5` removed `ActorBadge`, its shared
initials/seed/tone imports, all `.studio-avatar` geometry and six tone mappings, the
forced-colors override and the obsolete admission comment. This is visible in the actual
commit diff, not inferred from a merged-PR status. A current production-source/CSS search
finds no remaining badge renderer, avatar palette or caller in Studio.

The current [Account owner](../../apps/cantos-studio/web/src/account_menu.rs) accepts
only presentation inputs and shell callbacks. `AccountTrigger` uses shared `Button(Text)`
with the decorative `IconName::Person`; the shell supplies `entry::Key::Account` in both
locales. One shared `Dialog` receives the generic title and localized close action.
The shell's account-availability, navigation-close ordering, session refresh,
reauthentication, credential clearing and logout owners are unchanged by this review.

There is no ActorBadge left to adapt. Reintroducing one would change the newer footer
design without a product requirement. Keep the neutral shared control and do not create
a no-op migration. `Session` supplies an opaque `actor_id`, not a display name, email or
photo ([Identity auth contract](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md)).
Neither empty/missing actors nor a change of actor or locale needs an identity-color
fallback: the current Account component API receives no actor input at all.
Theatre's [ProfileAvatar](../../apps/cantos-theatre/web/src/features/guest_identity.rs)
instead consumes a Server-authorized `AccountPresentation`; it grants Studio no profile facts.

## Actual admission, not the historical comment

Looked up catalog `avatar` / `Avatar`, including initials-single, initials-pair, no-initial,
image, image-failed, sizes, named, theatre-control and generated-alias. Registration status
and its historical reason are not qualification. The actual `qualifications` pointers are:

- [Leptos Avatar](ui-requalification-737-avatar-leptos.json), with the actual commands,
  opened-capture and driven-input scope in [the #737 record](web-native-submit-737.md).
- [CMP Avatar](ui-requalification-615-avatar-cmp.json), with the independently executed
  JVM tests, inspected captures and device/AT limits in [the #615 record](ui-requalification-615.md).

Both existing records say PASS for structural/runtime/accessibility/visual and match the
current `python3 scripts/ui_preview_catalog.py --digests` output:

| Input | Current digest |
| --- | --- |
| Tokens | `79294511da18b463bd20056aa70cda914d488c43d6ccaabf0b398184b4fc1218` |
| Leptos source | `e92cc3969ad8f6b2dd7947917bb21ab48c01e63fab2fdb078428c0bf8053590b` |
| CMP source | `f88fcfc1645899658ad5aff58259ca5a263f4f0b09a3dac4e62e03b08149fe7a` |

The [shared adoption policy](../../packages/ui-system/shared/README.md#preview-registration-and-adoption)
freezes exact compatibility import sites at `209e9e90`. Studio `account_menu.rs` has no
baseline exemption for Avatar. A new import would require both fresh records; those records
are currently present, so lack of admission is **not** the current blocker. The repository
gate validates actual evidence freshness and guarded adoption. No record, digest, token,
generated output or baseline is changed here. Future adoption must repeat that check against
its own inputs and qualify its Studio flow; existing gallery qualification alone is insufficient.

## Regression scope and acceptance

The added `account_chrome_names_the_action_in_both_locales_without_a_profile` renders the
actual shared Account composition for VI/EN and requested open/closed states. It asserts
generic action/title/close copy, the controlled trigger state and dialog labelling, decorative
artwork, and absence of local/shared avatar, tone, initials or image markup. The existing
two-trigger/one-dialog test remains. This is native HTML/component evidence; native string
rendering deliberately leaves Dialog closed and does not exercise browser focus or dismissal.
The initial test draft used a shortened close-label expectation; its failed run is retained,
and the final assertion uses the actual existing localized resource. Production copy did not change.

Shared Avatar's existing independent vectors cover empty and punctuation-only names,
single/pair Vietnamese initials and combining marks, stable seed precedence, FNV tones,
semantic sizes, decorative/named semantics, image fallback and refused image URLs. These
belong to shared Avatar, not a restored Studio badge. The 48-unit control role remains
shared; the retired Studio medium/large badge mapping has no current consumer.

| Criterion | Decision / remaining evidence |
| --- | --- |
| AC-01 | Source/admission inventory complete at the fetched base; historical comment no longer applies. |
| AC-02 | Keep unchecked: the prerequisite renderer was retired by #582, so an adapter is not implemented or claimed. A future badge requires an explicit Studio consumer decision, current qualification and flow checks. |
| AC-03 | Generic VI/EN account semantics covered; shared vectors remain independent. Retired actor-tone/rename/size behavior is not relabelled PASS. Keep the combined criterion unchecked pending owner reconciliation. |
| AC-04 | No runtime markup/style delta. Prior footer observations retain their original source; no new header/popover, forced-colors, 200% zoom, focus/Escape/return or screen-reader acceptance is claimed. Keep unchecked. |
| AC-05 | Current consumer/admission/token decision, source history and actual local results recorded here and synchronized to the issue. |

No product rule, state/guard/outcome, contract, permission, model or Kani kernel changes.
Behavior SSOT impact: none; this is an existing-consumer audit plus native HTML regression.
L1/L2 are source-inspected; L3 checks are reported by the PR's final preflight. No new L4/L5,
browser, native-device, production, performance, motion or pixel-comparison result follows.
UI intensity remains Low, Studio Web only; token and localization values are unchanged.
No new images were downloaded from Drive. Raw logs, checked-tree identities and cleanup
receipt stay in ignored `artifacts/agent-runs/issue742/`. Cargo output is cleaned at handoff.

The remaining acceptance owner is unassigned: assignment and reconciliation of the retired
badge-specific criteria are required before closing #742. If a badge is requested again,
use the existing shared Avatar API and current two-renderer admission, then exercise the
actual Studio flow through `cargo xtask app cantos-studio --web` with a prepared local API.
No profile API, deployment, CMP product or unrelated session change is part of this PR.
