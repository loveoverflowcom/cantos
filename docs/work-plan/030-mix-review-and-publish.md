# Mix, review, upload, and publish an episode

## Why

A creator needs a finished episode that combines voices with ambient sound, music, and effects. Publication must expose a complete, approved version while failed mixing or upload leaves the current published episode playable.

## Scope

Extend Studio with a compact scene timeline, source/clip placement, mixing preview, corrections, and a clear QC/approval flow. Mix scenes and the episode; normalize loudness and convert to the delivery format using documented measurable targets. Use licensed or creator-supplied sound assets and preserve their permission metadata.

Upload scripts, voice/source assets, rendered scenes, and final episodes to S3-compatible storage. Provide CDN delivery and a publication manifest tying media to a particular script/casting/mix version. Automate the pipeline through upload; enable publication only after the required QC and approval records succeed. Support partial remixing and republishing.

Retain PostgreSQL job/version state and make publication changes atomic from a listener's perspective. Published content is generated once and streamed as stored media; listeners do not trigger TTS.

## Non-goals

A professional multitrack DAW, adaptive per-listener mixes, CDN multi-region orchestration, advanced moderation automation, or microservice decomposition.

## Dependencies

[020 — Cast characters and generate dialogue durably](020-cast-and-generate-dialogue.md). Inspect actual generated formats, durations, job behavior, and asset ownership before designing media tooling or storage contracts. Confirm storage/CDN credentials and choose concrete loudness, peak, and delivery-format targets.

## Suggested sequence

1. Mix a real multi-speaker scene with music/ambient/effect cues and inspect the output.
2. Add episode assembly, normalization, conversion, and objective QC records plus creator preview.
3. Implement resumable asset upload and durable storage references.
4. Add approval, publish, and republish behavior with a complete versioned manifest.

## Acceptance criteria

- A sample audio drama contains multiple voices, narration, ambient/music/effect cues, and a playable episode preview with measured loudness/peak results.
- Changed clips invalidate affected scene/episode renders; unchanged eligible scenes are reused. Old approved versions remain traceable.
- Missing clips, corrupt audio, clipping/target violations, unresolved blocking QC, absent publication permission, or missing required approval prevent publication.
- Upload interruption resumes safely or retries within limits. Missing uploaded objects and storage/CDN errors are visible rather than producing a published broken link.
- Publication exposes the new manifest only after all required assets and checks are complete. Failed republishing preserves the previously published version.
- Published media supports the seeking/delivery behavior required by the next player slice. Verify playback through the intended delivery path, not only a local file.
- A creator can preview, correct, rerender an affected portion, approve, and republish with recorded provenance and production costs.

## Review boundary

Demonstrate saved script → generated clips → mix → QC → upload → approval → publication using real sample audio and object storage. Verify failure recovery and publication atomicity. Review sound quality manually alongside measurable checks; document remaining limitations.

## Risks / unknowns

Mixing targets and cue interpretation require listening tests. Storage operations and database transactions cannot be assumed atomic together. CDN caching, byte-range support, and signed URL lifetimes need verification. Define how old media versions remain available for in-progress playback and offline clients before republishing.

## Follow-ups

Defer elaborate effects editing, automatic music composition, automated perceptual QC, and CDN optimization. Broken delivery or permission/approval bypass must be corrected before listener apps are released.

## GitHub execution

[#9](https://github.com/loveoverflowcom/cantos/issues/9), [#10](https://github.com/loveoverflowcom/cantos/issues/10), [#11](https://github.com/loveoverflowcom/cantos/issues/11). See the [issue/dependency map](project-planning.md) and [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) for the recommended sequence and live execution state.

Groundwork for #9: [#21](https://github.com/loveoverflowcom/cantos/issues/21) adds a standalone
WAV PCM inspection CLI ([evidence](../evidence/wav-pcm-inspection.md)) that reports structure,
checksums and sample peak. It chooses no audio profile or target, records no QC result and does
not satisfy any acceptance criterion above; #9 still decides whether to adopt it.

The [audio drama quality plan](audio-drama-quality.md) adds further groundwork without changing
#9's blockers: a rights-recorded sound asset library ([#46](https://github.com/loveoverflowcom/cantos/issues/46)),
sound bible and cue sheet records ([#47](https://github.com/loveoverflowcom/cantos/issues/47)), the
audio profile decision ([#51](https://github.com/loveoverflowcom/cantos/issues/51)) and clip
conditioning ([#52](https://github.com/loveoverflowcom/cantos/issues/52)). Scene acoustics and
placement ([#53](https://github.com/loveoverflowcom/cantos/issues/53)) follow #9's baseline mix.
