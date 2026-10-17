# Localization guard delivery

Invariant: added UI literals and feature-local locale maps in the five requested UI scopes must use an existing reviewed copy owner; changed bilingual owner files must retain vi-VN/en pairs and matching named placeholders (including occurrence counts).

Owner/boundary: `scripts/check_localization.py`, Python standard library only; integrated into existing `scripts/check_repository.py` repository gate after focused tests passed. No new xtask command, dependency, CI job, runtime catalog/library or application code. `.github/workflows/repository-checks.yml` repository checkout alone now has `fetch-depth: 2` so the existing gate has the PR merge first parent/prior develop commit. No GitHub CI action/status/run was read or executed.

## Selection and supported syntax

- Five exact prefixes: Theatre Web, Studio Web, Theatre Mobile, shared Leptos and shared CMP. `.rs/.kt/.html/.xml` only. Kotlin test source sets, Rust integration-test directories and `#[cfg(test)] mod` contents are diagnostics/fixtures rather than production UI.
- Local diff compares merge-base with `origin/develop` or `develop`, so branch commits, staged, unstaged and untracked additions are included. When the base equals HEAD locally only working additions are selected. Explicit `base` can be supplied to the primitive by tests.
- With no usable develop ref, parent fallback is restricted to known `GITHUB_ACTIONS=true` PR merge ref or develop push/dispatch. A PR merge must have two parents. Unknown/shallow ancestry without the parent fails with an actionable fetch-history error, never a silent empty check. On develop pushes with the fallback, the inspected change is the immediately preceding commit; PR merge checks cover the whole PR against first parent.
- Displayed literal detection uses tokens/comment/string masking and balanced `view!`/markup context, not a repository-wide regex ban. Technical native HTML attributes (including form `name`) stay technical; PascalCase component `name`, visible labels/tooltips/aria attributes and child strings are sinks. Named interpolation tuple keys are protocol.
- Kotlin finite sink inventory follows existing shared APIs: text, AccessibleName/DisabledReason constructors (including `.of`), Button, TextField, NumberField, choice controls, Select, FieldHelp, Tooltip, Dialog and SelectOption. It checks known visible positional/named parameters; model `value`, stable option IDs and callbacks stay technical. Semantics contentDescription/stateDescription/paneTitle are also checked.
- Existing exact-path owners are reviewed in `scripts/localization_allowlist.json`. Production owners are preserved. CMP PreviewToolCopy is an explicit fixed-English developer-tool owner; Leptos preview/messages combines fixed English tool constants with paired localized specimens.
- Whole changed bilingual files support existing resources! pairs, typed tuple match arms (including Rust block returns), reverse locale/key specimen matches, Studio boolean Copy structs/direct branch adapters, Kotlin paired enum records, ShellText override pairs and the launch XML explicit `_en` IDs. Nonblank translations and named-placeholder multiset parity are checked.
- Newly added unreviewed locale maps/resources macros fail. Direct raw error/API prose properties/to_string at known UI sinks fail. Direct typed error/status-to-Key/Text catch-alls fail; nested integer HTTP-code defaults are permitted.

## Reviewed exceptions

49 exact `(path, literal, complete source-line context, reason)` entries; no screen blanket exemption. They cover immutable revision/ID notation and punctuation, model/choice protocol IDs passed to localized owner lookups, product proper name, technical registry descriptors, preserved Vietnamese typography/accessibility specimens and separately English preview event diagnostics/noscript fallback. Changing a literal or context invalidates its exception. Owners confirmed their respective fixture/protocol/diagnostic classifications.

## Actual focused evidence

- `python3 -m unittest discover -s scripts -p test_localization.py` -> 39 tests PASS.
- `python3 -m unittest discover -s scripts -p test_check_repository.py` -> 111 tests PASS.
- `check_localization.repository_failures()` against the working final UI diff -> no failures, printed `Localization final-diff primitive PASS`.
- Failing-first probes included missing initial primitive, missing workflow parent contract, `<` comparison suppressing view strings, known CMP wrapper labels and accessibility `.of` constructors, native input `name` false positive, repeated-placeholder mismatch, changed interior multiline raw/HTML/a11y strings, and the existing `if en` adapter spelling. Each counterexample failed before its targeted repair, then passed.
- Diff-selection tests cover branch committed + staged + unstaged lines, untracked additions, known-parent fallback, unavailable base and scope exclusion. Negative fixtures cover resources, CSS/IDs/protocol, logs/comments/assertion/test fixtures, named parameters and nested numeric HTTP status fallback.

Evidence levels: example-tested and statically-checked within the finite supported syntax. Root owns final xtask preflight; do not call this primitive PASS a separate xtask gate result.

## Explicit residual scope

This is not a Rust/Kotlin compiler or arbitrary dataflow analyzer. Indirect strings assigned outside known UI calls, unknown wrapper APIs, arbitrary dynamic status/error completeness, prose hidden in model fields and external content cannot be proven by this scanner. Compiler-exhaustive enum mappings, resource/presentation tests and the manual audit own those cases. Kotlin String-returning `text(when(...))` mapping completeness remains compiler/tests evidence, while direct Key/Text-returning catch-alls are statically caught. Existing owner APIs are not migrated. No runtime locale switching, preservation, browser semantics, screen reader, device, plural/CLDR correctness or safe interpolation execution claim comes from static PASS. No language architecture decision #167/#168/#169 is accepted by this tooling.
