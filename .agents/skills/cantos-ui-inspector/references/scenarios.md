# Scenario contracts, fixtures and test accounts

> **Scope.** The schema of a reviewed inspection scenario, how to reuse, create, version and
> promote one, and how fixtures and test accounts are handled. **No runner is implemented.** The
> scenarios under [`../scenarios/`](../scenarios/studio-script-editor-v1.json) cover broad journeys,
> not every available mock/preview capability. Inspect the selected launch path and actual code:
> a live partial run may exercise implemented checks while missing required journeys remain
> blocked. PNG/metadata/semantics and comparison bundles are usable in `existing` mode; they
> never satisfy native or interaction checks.

## Where scenario data lives

| Data | Location | Shared through |
|---|---|---|
| reviewed scenario contracts | `.agents/skills/cantos-ui-inspector/scenarios/<id>.json` | Git, by a reviewed PR |
| an unreviewed draft written during a run | `artifacts/ui-evidence/<run-id>/scenario-draft.json` | local only (`/artifacts/` is ignored) |
| unreviewed scenarios kept between runs | `.local/ui-inspector/scenarios/<id>.json` — **only after** `.local/` is ignored | local only |
| the binding of a run (origin, build, account alias, device) | the run's report | local; shared only with confirmation |

A scenario never contains an origin, build ID, account alias, credential or signed URL; those
belong to a run.

## Schema version 1

| Field | Type | Required | Meaning and rules |
|---|---|---|---|
| `schema_version` | integer | yes | `1` |
| `id` | string | yes | `<surface>-<flow>-v<N>`, equal to the file name without `.json` |
| `status` | string | yes | `awaiting-implementation` (surface not built) · `needs-environment-binding` (surface exists; launch, fixture or account recipe missing) · `runnable` (a recorded run bound it successfully) · `retired` |
| `surface` | string | yes | `studio-web` · `theatre-web` · `cmp-theatre` |
| `renderer` | array of strings | yes | every renderer the scenario may run on: `browser`, `android-device`, `android-emulator`, `ios-device`, `ios-simulator`, `desktop-host`; the run records the exact one and its version |
| `purpose` | string | yes | one sentence: the journey and the risk it covers |
| `contract_refs` | array of strings | yes | repository-root paths with anchors of the docs that own the expectations |
| `preconditions` | array of strings | yes | environment class, role, data state, implemented capability |
| `fixtures` | object | yes | original or permitted content with its rights status; Vietnamese text exactly as it should render |
| `viewports` | array of objects | yes | `name`, `width`, `height`, `unit` (`css-px`, `dp` or `pt`), optional `density` and `note` |
| `themes` | array of strings | yes | `light`, `dark` |
| `locales` | array of strings | yes | BCP 47 tags; the locale set follows the [localization decision](../../cantos-ui-design/references/localization.md) |
| `font_scales` | array of objects | yes | `name`, `value`, `mechanism` (browser zoom, OS font size, Dynamic Type) |
| `input_modalities` | array of strings | yes | `pointer`, `keyboard`, `touch`, `screen-reader` |
| `matrix` | array of strings | yes | required combinations as `viewport/theme/locale/font_scale/input` — never the full Cartesian product |
| `steps` | array of objects | yes | `id`, `action` (intent in plain words, not selectors), `expected` (from the contract), `ref` (the owning doc anchor), `evidence` (the minimum level from the [foundation vocabulary](../../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)), optional `capture` |
| `states_to_capture` | array of strings | yes | named states captured with context and detail |
| `forbidden_actions` | array of strings | yes | additions to the skill's [forbidden list](../SKILL.md#3-budget-and-forbidden-actions); nothing here can remove one |
| `device_only_checks` | array of strings | `cmp-theatre` | checks only a named device, emulator or simulator can settle; a desktop host never satisfies them |
| `budget` | object | yes | `minutes`, `ui_actions`, `images` |
| `cleanup` | string | yes | what to retain and what to remove; never delete what the run did not create |
| `open_questions` | array of strings | no | product decisions an expectation depends on; a step that depends on one reports `inconclusive` instead of inventing the answer |
| `component_hints` | array of objects | no | seeds for [decomposition](components.md#2-discover-components--the-first-analysis-of-every-captured-state): `id`, `name`, optional `parent` and `note`; never an expectation, a complete taxonomy or a source locator |
| `design_references` | array of objects | no | design references a requested comparison may use: `component`, `reference_id`, optional `note`; the run pins revision, hash and approval status; listing one never triggers a comparison or makes the reference approved |

The repository check validates JSON syntax and rejects duplicate keys. No schema validator
exists; reviewers check the fields above by hand.

The two component fields are optional additions to schema version 1: every existing reviewed
scenario stays valid unchanged, and every inspection decomposes its captured states whether or
not a scenario has hints. Component targets and the design-comparison request belong to the run's
[scope](../SKILL.md#1-pin-the-scope), not to the scenario. Adding hints or references to a cited
scenario changes no step, expectation or fixture, so it is an editorial change; changing a step
or expectation for component work needs a new `-vN`.

## Rules for a scenario

| Rule | Failure it prevents | Good / counterexample | Oracle · status | Exception |
|---|---|---|---|---|
| expectations come from the product contract, never from current behavior | a bug enshrined as the expected result | "the mini-player keeps the current episode across navigation" ([UI system](../../../../docs/design/ui-system.md#theatre-compact-listening)) / "the player closes on navigation, as it does today" | each `expected` has a `ref` that says so · manual | none |
| steps state intent, not selectors | a renamed CSS class reads as a product regression | "open the speed control and choose 1.5×" / `click("#spd > li:nth-child(4)")` | review · manual | a future runner keeps its bindings beside it, outside the contract |
| fixtures are original or permitted and realistic | private manuscripts in Git; diacritic bugs hidden by ASCII text | `Ngày mai, mình có diễn tiếp không?` / "Lorem ipsum", a real author's chapter | review · manual | none |
| a cited contract is never edited in place | old reports point at expectations that changed under them | a new `-v2` file and `retired` on v1 / editing v1's `expected` after a report cited it | review of the diff · manual | editorial fixes that change no step, expectation or fixture |
| no secret or environment binding | credentials or QA origins leak through Git | `account role: creator` / an alias with a password, a signed URL | review · manual | none |
| no paid or publishing step | an audit charges a provider or releases an episode | inspect "Publish episode" disabled with its reason / a step that clicks it | review · manual | none |
| component hints seed, never decide | a stale hint hides a new component or turns a missing one into a pass | `{"id": "search-dock", "name": "Search Dock"}` / a hint with `"expected": "passes"` or a guessed source file | review · manual | none |

## Reuse before creating

1. Search the [reviewed scenarios](#reviewed-scenarios) by surface and flow. Reuse the closest
   one and record the run-time binding in the report.
2. If its expectations do not cover the requested question, add steps in a **new version** —
   never by editing a cited one.
3. If none fits, draft a scenario in the run directory with expectations from the owning docs
   and every unresolved product question in `open_questions`.
4. Promotion into `scenarios/` is a repository change: reviewed, through
   [`cantos-work-item`](../../cantos-work-item/SKILL.md), committed only when the user asks.

## Fixtures

- Prefer the original content of the [example episode](../../../../services/cantos-server/contracts/studio/script-ir/examples/episode-draft.json)
  (`Ánh đèn cuối sân khấu`, `Một lời hẹn`, `Người dẫn chuyện`, An, Minh) and extend it with
  original stress text: long names, stacked diacritics, long titles, empty libraries.
- Seed data only through the app's official flows or a seed the repository provides; never edit
  a database directly to reach a state.
- Keep the fixture's rights status honest. `pending_review` is useful: publication must stay
  blocked, and the blocker must be explained.
- Record every fixture ID created or changed by a run in its report, and what was retained.

## Test accounts and credentials

Accounts exist only in a local or QA environment the user authorized, created through the
official sign-up flow. Proposed local layout, usable **only after** `.local/` is ignored by Git
(confirm with `git check-ignore -v .local/ui-inspector/accounts.json`):

```text
.local/ui-inspector/            directory mode 0700, created with umask 077
  accounts.json                 metadata only, never secrets
  credentials/<alias>           one file per account, mode 0600, no symlinks
  scenarios/<id>.json           unreviewed scenario drafts
```

```json
{
  "schema_version": 1,
  "accounts": [
    {
      "alias": "qa-creator-local",
      "surface": "studio-web",
      "environment": "local",
      "role": "creator",
      "credential_ref": "credentials/qa-creator-local",
      "status": "unprovisioned",
      "created_by_run": null,
      "last_validated_at": null,
      "allowed_actions": ["edit owned QA scripts", "save owned QA drafts"],
      "cleanup": "retain for reuse; remove only fixtures this registry records as owned"
    }
  ]
}
```

This is an example, not an existing account. Procedure:

1. Read metadata first; never print a credential file.
2. Provision once: record `provisioning` with the run ID, generate the password locally and write
   it directly into the credential file, sign up through the official UI, then set `ready` only
   after the signed-in identity and role are confirmed in the app.
3. A timeout after sign-up is `unknown-outcome`: reconcile through an official sign-in before any
   second attempt. Never create twice blindly.
4. Expired session → one official sign-in attempt. Role mismatch → `blocked`; never escalate
   privileges. MFA or passkey → hand off to the user; never bypass.
5. Never pass a password in a command argument, a step, a scenario, a report or a screenshot. A
   saved browser storage state is a credential.
6. Never rotate or overwrite a shared account as a recovery shortcut. Across machines, use
   separate aliases.

## Reviewed scenarios

| ID | Surface | Covers | Status |
|---|---|---|---|
| [`studio-script-editor-v1`](../scenarios/studio-script-editor-v1.json) | Studio Web | speaker correction, validation errors, draft and saved state, revision identity, 320 CSS px reflow, 200 % zoom, keyboard-only editing, light/dark | `awaiting-implementation` |
| [`theatre-web-player-v1`](../scenarios/theatre-web-player-v1.json) | Theatre Web | discovery with a missing cover and long titles, mini and full player, seek, speed, bookmark, reload and resume, interruption and expired access, no production jobs on play | `awaiting-implementation` |
| [`cmp-theatre-playback-v1`](../scenarios/cmp-theatre-playback-v1.json) | CMP Theatre | native playback, background and lock screen, interruption and route change, sleep timer, cold-launch resume, large text, TalkBack and VoiceOver | `awaiting-implementation` |
