# Script and editor acceptance reference

Current requirements for maintaining the existing Script IR/import/editor boundary. This is
an acceptance reference, not a request to reimplement completed #1/#4 or a claim of fresh runtime
verification. The [original 010 delivery plan](../archive/superseded-plans/010-import-and-edit-script.md)
is historical. Actual compatibility belongs to [Script IR](../architecture/script-ir.md) and
the [versioned contracts](../../packages/contracts/README.md).

## Scope

Preserve original manuscript bytes and extraction uncertainty; support the implemented bounded
TXT/Markdown/DOCX/Script IR intake. An external host authors adaptation proposals; Cantos
validates, records provenance and lets Studio compare/edit/accept immutable revisions.

Stable scene/dialogue/speaker identities, exact source bindings, rights references and revision
history survive edits. Optional structured scripts do not make Catalog narration or an
unscripted podcast require a script/cast. Studio remains Web-only.

## Non-goals

Backend inference/TTS/mixing, arbitrary document conversion, universal narrative schemas,
real-time collaborative editing and direct Narrative Forge runtime/storage coupling.

## Acceptance criteria

- Supported formats produce inspectable preserved-source/extraction results; malformed or
  unsupported input cannot create a misleading ready script.
- Studio can edit scenes/dialogue/speakers, review an externally authored proposal and reopen
  its accepted revision after restart, within the actual implemented contract.
- Saving creates a successor revision; prior versions and competing edits are not silently
  overwritten. Interrupted or uncertain saves preserve the source and last valid revision.
- Duplicate IDs, unresolved speakers, invalid references and missing required metadata fail
  with the exact documented errors, rather than entering production silently.
- Provider/usage/rights statements are attributed claims, not measured costs or legal approval;
  missing publication rights remain a release blocker.
- Correct Vietnamese Unicode/pronunciation distinctions, keyboard access, accessible contrast
  and narrow-screen behavior retain their independent source/runtime evidence.

## Risks / unknowns

DOCX extraction can lose structure; compare with preserved source. AI proposals may omit or
invent content; source provenance and editorial review remain necessary. Production issuer,
legal/listening quality and full accessibility acceptance require their own observations.

The [editor evidence](../evidence/studio-script-editor.md), [adaptation evidence](../evidence/ai-script-adaptation.md)
and [revision evidence](../evidence/script-revision-persistence.md) are historical results for
named inputs. Re-run only checks affected by the current change under the repository cadence.
