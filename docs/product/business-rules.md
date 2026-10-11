# Business Rules

> Status: target product contract. Script IR structure, reference and text rules have an
> [implemented validator](../../packages/contracts/README.md). Local immutable revision persistence
> and actor/read/write checks have [bounded evidence](../evidence/script-revision-persistence.md);
> [Production-input planning](../evidence/casting-production-inputs.md) implements bounded
> reference-only casting, rights/budget declarations and immutable candidate/approval checks.
> Legal clearance, actual dispatch/cost settlement and publication gates remain separate work.

## Content and revisions

1. Preserve the editorial hierarchy: **Work → Adaptation → Episode → Act → Scene → Dialogue → Audio**. Narration is a spoken role; ambience, music and effects are scene cues linked to audio assets.
2. Support original scripts, imported manuscripts and AI-assisted adaptations. Preserve the imported source and record its provenance; adaptation must be reviewable before production.
3. Give editorial entities stable identifiers. Editing creates a new immutable script revision once a revision is submitted for production; in-progress drafts may be mutable. A production run pins one script revision and its resolved casting, voice settings, pronunciation rules and sound assets.
4. Maintain a versioned Script IR with schema validation and explicit migration/compatibility rules. Reuse Narrative Forge concepts through a documented exchange format, without a mandatory dependency on that project's storage or deployment.
5. Do not mutate approved or published output in place. Corrections produce a new revision, production run or release as appropriate. Retain the previous release and its audit history according to the retention policy.

## Casting and performance

6. A character has a stable identity, personality, voice assignment, pronunciation guidance and emotional/performance profile within an adaptation. Changes must be explicit and versioned.
7. A line may override permitted performance settings, such as emotion or pacing, without silently changing the character's voice identity. Record resolved settings in the production input.
8. Voice selection and provider capabilities must be validated before production. Missing voices, unavailable models or unsupported required settings block the affected work with an actionable finding; do not silently substitute a different voice.
9. Source text and pronunciation rules must remain available for inspection. AI-inferred speakers, emotions and cues are suggestions until the script/casting review accepts them.

## Rights and access

10. Record permission evidence, rights holder/source, applicable scope, language, territory, term and attribution requirements where relevant for source works, adaptations, voices, music and effects. Imported material is not automatically cleared for publication.
11. A creator must have the necessary rights for the intended production and distribution. Unknown or expired permissions block the relevant operation. Configure checks for the required rights before paid generation and again before publication.
12. Generated outputs inherit applicable provider and source restrictions; an output's existence does not establish its distribution rights. Provider credentials remain server-side and must not appear in scripts, manifests or logs.
13. Define permissions for script editing, production execution, QC acceptance, publication approval and listener access. The implementation may initially allow one authorized creator to perform several roles, but each approval must record its actor and scope.

## Production, caching and costs

14. Generate dialogue audio once for a specific input fingerprint. Include exact effective text, voice/provider/model version where available, synthesis settings, pronunciation version and relevant adapter/processing version in the key. Record actual provider identifiers and any unversioned-model uncertainty.
15. Cache reuse is allowed only when the input fingerprint, access/rights scope and artifact integrity remain valid. Do not reuse private voice assets across unrelated creators. An uploaded result is immutable; revised inputs produce a new artifact.
16. Regenerate only changed or invalid dialogue and its dependent scenes/episode mixes. A voice or pronunciation change invalidates every line that depends on that version. Unrelated lines remain reusable.
17. Track estimated cost before execution and actual usage/cost per generation attempt, production run and provider. Cached reuse, failed calls, retries and credits must be distinguishable. Never present an estimate as a settled charge.
18. Enforce configured production budgets and concurrency limits. If a limit is reached, pause or fail with an explanation; automatic retries must not bypass the limit. Provider billing reconciliation is needed when actual cost arrives later.
19. Background jobs must survive worker restarts. Duplicate dispatch or a retried step must not create duplicate accepted output. A paid provider call with an unknown outcome requires reconciliation or explicit handling before an equivalent call is repeated.

## Review and publication

20. QC must cover the configured technical profile and editorial concerns: missing/wrong dialogue, unsuitable pronunciation or emotion, audible artifacts, clipping, duration anomalies and unbalanced speech/music/effects. Automatic checks support human review; passing technical checks alone does not establish editorial quality.
21. Approvals apply to a specific script/render revision. Any relevant edit, recast, changed asset or replacement render invalidates affected approvals. Preserve who approved what and when.
22. Automatic upload may follow successful production. Automatic publication is permitted only after all required rights checks, QC results and approvals for that exact release are satisfied. A pending approval or unresolved blocking finding prevents publication.
23. A published release references a complete immutable asset manifest and verified deliverable files. Partial uploads, intermediate dialogue artifacts and unapproved previews must not become publicly discoverable or downloadable through Theatre.
24. Republishing updates the active release pointer only after the replacement is ready. Retraction must remove access/discovery according to the distribution policy; previously downloaded copies cannot be assumed remotely erased.

## Listening and mobile

25. Theatre streams previously produced, published files through the configured storage/CDN path. A play request must not start adaptation, TTS or mixing.
26. Native Android/iOS playback must support background audio, lock-screen controls, notifications where applicable, playback speed and sleep timer. Mobile audio cannot depend on a WebView.
27. Downloads must be opt-in, resumable and integrity-checked, subject to publication/access rules and device storage limits. Document what happens to downloaded releases after replacement, expiry or withdrawal.
28. Store progress and bookmarks by user, episode and release context. Define conflict resolution so a delayed offline device cannot silently overwrite newer progress from another session. Playback completion and explicit restart/rewind are distinct events.
29. Apply accessibility requirements across all supported surfaces: readable contrast, reduced-motion support, semantic labels, keyboard navigation on Web and assistive-technology support on mobile.

## Product invariants to verify

| Scenario | Required result |
| --- | --- |
| One line changes | Only that line and dependent mixes need regeneration |
| Character voice changes | All dependent lines are invalidated; unrelated characters' audio remains eligible for reuse |
| Worker dies after dispatch | A recovered run resumes safely without duplicate accepted output |
| Provider response is lost | Reconcile the uncertain paid call before repeating it |
| QC approval predates a new render | Publication remains blocked until the new render is reviewed |
| Upload is incomplete | No incomplete release becomes listener-accessible |
| Listener presses play repeatedly | No synthesis cost is incurred |
| Offline progress arrives late | The documented sync policy resolves it without blind last-arrival overwrite |

Related: [product brief](brief.md) and [production pipeline](production-pipeline.md).
