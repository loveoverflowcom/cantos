# Clean, readable syntax

> **Scope.** Write Cantos code that a reviewer understands in one reading: naming, function
> shape, control flow, error handling, comments, imports, module layout and formatting in Rust,
> Leptos `view!` markup and Kotlin/Compose. Use while writing or reviewing any code. Formatting
> tools are not configured yet; this reference states the style the first toolchain setup should
> enforce.

Clean here means *intention-revealing and predictable*, not short. Never compress code with
clever tricks, wildcard imports or semicolon chains to make it look smaller.

## Naming

| Kind | Rule | Good | Avoid |
|---|---|---|---|
| Transformation | verb naming the data flow | `parse`, `validate`, `resolve`, `decide`, `plan`, `reduce`, `apply`, `fingerprint`, `project`, `encode` | `process`, `handle`, `manage`, `do_stuff` |
| Domain type | domain noun from the docs | `ScriptRevision`, `ProductionRun`, `ProviderAttempt`, `ReleaseManifest` | `Data`, `Info`, `Manager`, `Helper` |
| Boolean | predicate | `is_current_for`, `has_blocking_findings` | `flag`, `check` |
| Test | theorem statement | `stale_approval_blocks_publication` | `test_publish_2` |
| Error variant | what went wrong, with data | `UnknownSpeaker { dialogue: DialogueId }` | `Error1`, `Invalid` |

Keep Cantos vocabulary exact: *Studio*, *Theatre*, *Work*, *Adaptation*, *Episode*, *Act*,
*Scene*, *Dialogue*, *cue*, *production run*, *attempt*, *artifact*, *release*, *manifest*.
Do not invent synonyms ("chapter", "line item", "build") for the same concept.

## Function shape

- One level of abstraction per function. A function that validates, decides and writes is three
  functions.
- Prefer early returns and `?` over nested `if let` pyramids.
- Prefer `match` over chains of `if`/`else if` on the same value; exhaustive, without `_ =>` on
  domain enums.
- Prefer iterator pipelines when they read as a sentence; use a plain loop when the pipeline
  needs comments to follow.
- Keep argument lists meaningful: group related facts into a named input struct rather than
  passing seven positional parameters, but do not bundle unrelated values to hit a count.
- Size is a signal, not a rule. Split by responsibility, never by line quota.

```rust
// Illustrative: reads as the rule it implements.
pub fn decide_publication(facts: &ReleaseFacts) -> Result<PublishableRelease, Vec<PublicationBlocker>> {
    let blockers: Vec<_> = [
        rights_blockers(&facts.rights),
        qc_blockers(&facts.qc_results),
        approval_blockers(&facts.approvals, &facts.candidate),
        asset_blockers(&facts.assets),
    ]
    .into_iter()
    .flatten()
    .collect();

    if blockers.is_empty() {
        Ok(PublishableRelease::new(facts.candidate.clone()))
    } else {
        Err(blockers)
    }
}
```

## Errors

- Rust: one error enum per module boundary (`thiserror` is a candidate), with structured data in
  variants. No `anyhow` or string errors in the domain; reserve them for binaries and top-level
  glue. No `unwrap`/`expect` outside tests and proven-infallible constants (explain those).
- Kotlin: sealed interfaces or `Result` for expected failures; exceptions only for programmer
  errors and platform boundaries; no `!!`.
- Never swallow an error. Logging and continuing is a decision that must be stated and tested.

## Comments and docs

- Comments explain *why*: the invariant, the trade-off, the external constraint. The code says
  *what*.
- Public domain types and functions get a doc comment stating the invariant or contract.
- No commented-out code, no TODO without an issue link, no "important for AI" notes.

## Imports and modules

- Explicit imports; no glob imports except a crate's documented prelude or test `use super::*`.
- Module roots read as maps: public types, invariants and ownership; details live in leaf modules.
- Group by feature (`script`, `production`, `publication`, `listening`), not by technical kind
  (`models`, `services`, `utils`).

## Leptos markup

- `view!` shows structure and semantics: semantic elements, localized text, ARIA only where HTML
  semantics are insufficient.
- Semantic class names backed by component styles; no walls of utility classes.
- Event handlers call a named function or dispatch a typed event; no business logic inline in
  the markup.

## Kotlin and Compose

- Official Kotlin style; `modifier: Modifier = Modifier` as the first optional parameter of a
  layout-emitting composable, applied once at the root.
- Callbacks named `onX`; state passed as values; no mutable collections in UI state.
- Multiline arguments and modifiers for readability; trailing commas where the formatter allows.

## Formatting and linting

Once toolchains exist, each owns one formatter and one linter, run in check mode before handoff:

| Toolchain | Formatter | Linter | Status |
|---|---|---|---|
| Rust | `rustfmt` (`cargo xtask verify rust`) | `clippy -D warnings`, native and WASM (`verify rust`, `verify web`) | current |
| Kotlin | candidate: ktlint | candidates: detekt, Compose Rules | proposed; record the choice in an ADR |
| Python scripts | standard library style, 4-space indent | none configured | current |
| Markdown/JSON/YAML | `.editorconfig` + `scripts/check_repository.py` | the same script | current |

Do not mix a mass reformat into a behavior change. Do not add a formatter configuration the
repository has not chosen; propose it instead.

## Review checklist

- Can each function be named after one transformation?
- Does every domain enum match exhaustively, and does every error carry its cause?
- Would a new contributor find the rule by its domain name?
- Is any line clever where it could be plain?
