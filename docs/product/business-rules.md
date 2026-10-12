# Business Rules

> Status: target product contract. Script IR structure, reference and text rules have an
> [implemented validator](../../packages/contracts/README.md). Local immutable revision persistence
> and actor/read/write checks have [bounded evidence](../evidence/script-revision-persistence.md);
> [Production-input planning](../evidence/casting-production-inputs.md) implements bounded
> reference-only casting, rights/budget declarations and immutable candidate/approval checks.
> Cantos does not synthesize audio: external producers do, and submit it back
> ([ADR 0018](../decisions/0018-external-audio-production.md)); the submission API is not yet
> implemented. Legal clearance, audio acceptance and publication gates remain separate work.

## Content and revisions

1. Preserve the editorial hierarchy: **Work → Adaptation → Episode → Act → Scene → Dialogue → Audio**. Narration is a spoken role; ambience, music and effects are scene cues linked to audio assets.
2. Support original scripts, imported manuscripts and AI-assisted adaptations. Preserve the imported source and record its provenance; adaptation must be reviewable before production.
3. Give editorial entities stable identifiers. Editing creates a new immutable script revision once a revision is submitted for production; in-progress drafts may be mutable. A production snapshot pins one script revision and its resolved casting, voice settings, pronunciation rules and sound assets.
4. Maintain a versioned Script IR with schema validation and explicit migration/compatibility rules. Reuse Narrative Forge concepts through a documented exchange format, without a mandatory dependency on that project's storage or deployment.
5. Do not mutate approved or published output in place. Corrections produce a new revision, production run or release as appropriate. Retain the previous release and its audit history according to the retention policy.

## Casting and performance

6. A character has a stable identity, personality, voice assignment, pronunciation guidance and emotional/performance profile within an adaptation. Changes must be explicit and versioned.
7. A line may override permitted performance settings, such as emotion or pacing, without silently changing the character's voice identity. Record resolved settings in the production input.
8. Voice selection and the declared capabilities of the named voice or model must be validated before a production snapshot is approved. Missing voices, unavailable models or unsupported required settings block the affected work with an actionable finding; do not silently substitute a different voice.
9. Source text and pronunciation rules must remain available for inspection. AI-inferred speakers, emotions and cues are suggestions until the script/casting review accepts them.

## Rights and access

10. Record permission evidence, rights holder/source, applicable scope, language, territory, term and attribution requirements where relevant for source works, adaptations, voices, music and effects. Imported material is not automatically cleared for publication.
11. A creator must have the necessary rights for the intended production and distribution. Unknown or expired permissions block the relevant operation. Check the required rights before production inputs are approved and again before publication.
12. Produced audio inherits applicable provider and source restrictions; its existence does not establish its distribution rights. Cantos holds no TTS provider credential, and no producer credential may appear in scripts, manifests or logs.
13. Define permissions for script editing, production execution, QC acceptance, publication approval and listener access. The implementation may initially allow one authorized creator to perform several roles, but each approval must record its actor and scope.

## External production and submission

14. Cantos does not generate audio. A producer outside Cantos (an AI plugin, an agent or a person with an audio tool) reads the approved production snapshot and the published standards, produces the audio and submits it through the API. Cantos supplies guidance and verifies what comes back.
15. A submission names the exact script revision, snapshot digest and dialogue IDs it covers, and carries the producer's provenance claim (tool, model or voice, version, rights assertion). An accepted result is immutable; a corrected result is a new submission. Producer claims are recorded as claims, never as facts Cantos measured.
16. Ask for new audio only for changed or invalid dialogue and its dependent scenes and episode mixes. A voice or pronunciation change makes every submission that depends on that version stale. Unrelated lines remain reusable. Do not reuse private voice assets across unrelated creators.
17. The producer owns provider accounts, credentials, spend and retries. Cantos makes no paid generation call and never presents a producer-reported cost as a settled charge. Producers authenticate with scoped, revocable credentials, never a creator's browser session.
18. Submitted audio is untrusted. Cantos inspects it (format, checksum, duration, sample peak) before it can be accepted, records those measurements separately from the producer's claims, and rejects unreadable, mismatched or oversized input. Passing inspection is not QC approval.
19. Submissions are idempotent. A repeated or interrupted upload must not create duplicate accepted output, an incomplete upload is never accepted, and selecting the accepted result for a dialogue is a recorded decision tied to its checksum.

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
| One line changes | Only that line's submission and dependent mixes become stale |
| Character voice changes | All dependent lines are marked stale; unrelated characters' audio remains eligible for reuse |
| Upload is interrupted or repeated | No duplicate accepted output; the incomplete upload is never accepted |
| Audio arrives for a superseded revision | It is rejected or marked stale, never accepted for the current revision |
| QC approval predates a new render | Publication remains blocked until the new render is reviewed |
| Upload is incomplete | No incomplete release becomes listener-accessible |
| Listener presses play repeatedly | No generation is started and no producer is contacted |
| Offline progress arrives late | The documented sync policy resolves it without blind last-arrival overwrite |

Related: [product brief](brief.md) and [production pipeline](production-pipeline.md).
