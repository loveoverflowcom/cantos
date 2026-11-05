# Studio preferences footer and Account (#582)

Source-bound implementation record, based on `develop@a75221f4e1b70fdbd87faf2edba5f6b59a3d0508`
after #131 / PR #748. This implements the utilities amendment in
[HANDOFF v3](https://drive.google.com/file/d/1nlZGJQnNMUShKjR624Zb0tE_WtF7h9ve/view).
The PR identifies the final source and checks. A local build is not a deployed build.

## Boundary and owners

The authenticated rail and drawer render Appearance, Language and Account in that order.
The header retains brand/context. Signed-out entry preferences remain available. One root
`Theme` signal and one locale owner feed all presentations; pane changes and chrome replacement
never create preference stores. Locale options use `Locale::supported()` and full autonyms;
canonical saved tags and storage-refusal feedback keep their existing implementation.
Theme initializes to System on reload, as before. No theme storage key or persistence layer is
added. The user-submitted implementation prompt explicitly retains this contract, clarifying
AC-06's older generic persistence wording: locale persists, theme remains session-owned.

The composition reuses catalog `select` / `Select` (compact native-option contract), `button`
(text), `icon` (neutral Person) and `dialog` / `Dialog`. No shared API, token, expression,
authentication or transport contract changes. A browser probe of the enhanced theme menu
reproduced dismissal after a locale change; this consumer uses the existing native Select
contract instead. It adds no feature-local combobox or shared qualification exception.

One `AccountEntry` lives outside navigation. Scoped footer triggers dispatch to it; drawer
closure precedes Account opening on the next animation frame. Shared native Dialog owns initial
close focus, background inertness, Tab containment, Escape and focus restoration. The visible
rail Account or Menu is the fallback after resizing hides an opener. Session refresh,
reauthentication and logout remain the shell's existing callbacks. Typed credentials still clear
when Account closes. No actor key is rendered as a name, initials, photo or profile fact.
Localized navigation chrome mounts closed before reopening and fences callbacks from replaced
instances. Preference values and the mounted workspace owners remain outside that lifetime.

No ADR: no new library, durable policy or framework is selected. Behavior SSOT impact: no
semantic change to session/domain guards; modal and preference presentation compose the existing
owners. No registered model or Kani kernel changes. The catalog-only unknown-impact obligation
is explained in the ignored assurance record. Queue: no change; #131 → #582 order is retained.

## Supersession and historical evidence

Footer v3 supersedes AC-01's topbar v7/v8 placement and AC-02's cycle presentation. Registry,
locale persistence, System media response, session and accessibility invariants remain.
[PR #590](https://github.com/loveoverflowcom/cantos/pull/590),
[PR #591](https://github.com/loveoverflowcom/cantos/pull/591),
[topbar v7/v8](studio-topbar-582.md) and [joint continuation](topbar-582-583.md) remain historical,
source-bound evidence. Their results are not assigned to this footer.

## Reproduction and evidence boundary

Full build/check logs, browser driver, synthetic fixtures, reports, captures, source/build hashes
and cleanup receipt remain in ignored `artifacts/agent-runs/preferences-582/`. Both baseline and
final app are debug CSR builds through `cargo xtask build cantos-studio --web`. Chrome serves the
exact retained static bytes on task-owned loopback port 18582. Playwright intercepts API calls
with synthetic Studio session/catalog/content responses; the production app and its adapters
run, but no real Identity, cookie session, server database or production account is provisioned.
Synthetic reauthentication credentials are not real secrets and are not logged.

The browser ledger records engine, viewport CSS pixels, DPR 1, zoom 1, selected theme, locale and
root font size. The 200% text case sets the root font to 32px; it is not actual browser page zoom.
System's light/dark/light CSS media response uses browser media emulation, not a physical OS
settings change. Before/after screenshot pairs use the same synthetic content and viewport.

An unrelated duplicate `live-review-confirm-help` remains in the unchanged publication review
owner. The footer, preference, navigation and Account IDs are unique; no duplicate Account or
nested navigation/Account modal is introduced. This record does not accept that other owner's
accessibility debt.

Real authenticated Server/Identity integration, deployed/production acceptance, screen reader,
OS IME, physical OS settings, actual browser zoom, touch and forced-colors acceptance remain
NOT_RUN. These are explicit issue acceptance gaps, not evidence inferred from synthetic HTTP.

## Observations and local gates

- Native Studio: `cargo clippy -p cantos-studio --all-targets --locked -- -D warnings` PASS;
  `cargo test --locked -p cantos-studio` PASS, 492 tests plus 5 doctests. Native HTML assertions
  cover canonical selected values, full locale names, scoped control IDs and one Account dialog.
- Final changed-only preflight: repository, Rust formatting, both Web WASM lint/builds, shared
  Leptos preview and the assurance decision PASS. The repository suite ran 1,384 Python tests
  with 20 explicit skips. The catalog conservatively broadens selection to every gate; the
  11 unrelated full-workspace/backend/CMP/audio/mutation/reference gates are explicitly NOT_RUN,
  not PASS. Studio native lint/tests above cover the changed Rust consumer separately.
- The final styling correction reserves header space above the rail so Account is visible in
  the initial desktop viewport. At short heights the Account dialog scrolls as a whole; its
  heading cannot consume the actions' scrolling area. The browser checks the complete Logout
  target after focus at 600×420 / 200% text. Final Web build and same-scenario capture cover
  these CSS-only changes after preflight; source/whitespace validation is rerun for the record.
- Browser observations use Chrome 156.0.8078.80, actual production app over synthetic HTTP.
  The final ledger covers 320/390/1440px, Light/Dark/System, VI/EN, native preference changes,
  canonical locale reload, System reload initialization, dynamic browser color preference,
  storage refusal, rail/drawer resize/pane transitions, modal focus/Tab/Escape/Close,
  reauthentication form clearing and adapter submission, refresh and logout.
- Native OS picker selection by synthetic keys remains NOT_RUN. A plain standalone HTML
  select in this same headless Chrome also ignored Arrow/Space/Enter sequences; this is
  independent of Cantos. Real Tab/Shift+Tab order and Account/drawer keyboard dismissal were
  observed. Selection callbacks were exercised through native DOM select changes.

Opened matching System/VI before-after views at 1440 and 390px, a Dark 320px drawer, Account
from the drawer, enlarged-text short-height states and the signed-out entry. The desktop footer
now leaves context in the header and shows full selections plus a neutral Account entry.
The 320px drawer wraps long Vietnamese navigation labels and keeps utilities in order. Other
captured cells have measured document-width checks but are not an all-element visual or
contrast audit. The final ledger supplies capture counts, hashes and individually opened files.

Self-review covered all handwritten changes, generated resource/digest changes and retained
session/form/locale/theme owners. No new dependency, service, migration, wire contract,
permission, durable record, publication action or provider cost is introduced. No deploy.
L1–L3 named presentation scope is verified; L4 browser observations use explicit HTTP doubles.
L5 real authentication, OS/native-picker and assistive-technology acceptance remains owed to
#582. Neither a merged PR nor historical screenshots close those gaps.
