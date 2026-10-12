# Cantos — Business Rules

> Current product contract. [ADR 0018](../decisions/0018-external-audio-production.md)
> replaces earlier assumptions that Cantos invokes TTS providers or mixes audio on its server.

## Work, chapter and episode identity

1. A work has an immutable ID and a declared type: novel, story or podcast.
2. Content units have immutable IDs and stable ordering within a work; chapter/episode text
   and Script IR are optional. Editing metadata never mutates stored audio bytes.
3. Revisions are immutable once accepted; publication points to exact revisions/objects.
4. A podcast episode can be submitted as one already-recorded file. Never require a scripted
   dialogue hierarchy for a simple story or podcast.

## Casting and performance

5. Script IR, narrator/cast, pronunciation maps, emotions, scene cues and audio mix instructions
   are **guidance for an external producer**. They are not proof a provider supports a control.
6. A missing required voice, rights claim or control produces a visible blocker/fallback
   decision. Never silently substitute a speaker, a language, or a voice.
7. Bind instructions and producer receipt to the exact reviewed content/brief revision; a new
   revision never silently alters the old brief or previously submitted media.

## Production, caching and costs

8. Cantos never dispatches a TTS request, runs provider SDKs or reserves vendor spending.
   External tools own their provider execution, retries, caches, budgets and usage accounts.
9. A producer can submit external idempotency IDs, provider/model identifiers, duration,
   cost and quality reports as **caller-declared provenance**. These are not Cantos charges or
   independently verified measurements.
10. Retries of an upload/finalization use a stable idempotency key and exact content digest;
    repeat requests must not create multiple accepted assets or attach to another episode.
11. Content/brief changes may mark a previous audio submission stale for review; the backend
    never auto-regenerates audio or instructs a provider to retry without an external request.

## Audio intake and quality

12. Uploads are private, scoped to one authorized work/content unit and capped in bytes and
    accepted MIME/container/codec according to a **versioned profile**.
13. A producer uploads directly to private object storage via a short-lived scoped grant.
    The API validates the declared object reference, actual stored bytes, size and digest;
    never trust a filename or client-declared digest alone.
14. QC report fields preserve whether they are producer-declared, server-inspected or
    human-listened. A structural pass or a valid checksum is not editorial approval.
15. Opus mono 20–40 kbps and AAC-LC mono fallback are candidates, not final constraints
    until the versioned profile is decided ([#64](https://github.com/loveoverflowcom/cantos/issues/64)).
    Original audio masters may be retained privately if storage policy permits.

## Rights and provenance

16. Record source/content rights and audio/voice/music/effect usage rights separately.
17. A submitted permission claim does not prove authorization. Expired/revoked/out-of-scope
    rights block new approval/publication; old published releases follow explicit takedown policy.
18. Preserve the producing agent, tool/version and source/brief revision when declared, with
    provenance visibility. Do not store provider secrets or entire private prompts in public
    listener responses.

## Publication and listening

19. A published release is immutable and references exact validated objects, rights and
    approval records. It never changes in place when a new upload arrives.
20. Publish a replacement atomically only after every referenced asset is present and accepted.
    Failure preserves the previous listenable release.
21. Theatre reads published audio and progress only. Streaming/offline replay **never**
    invokes TTS, a model, a mixer or an audio production job.

## Security and access

22. Producer authorization is independent of the Studio browser session. Use scoped credentials
    and temporary upload grants, least privilege, owner/work boundaries, expiry and revocation.
23. Validate input sizes, MIME and actual signatures, digests, request identifiers and ownership
    under bounded resource use. Never accept untrusted paths as filesystem destinations.
24. Separate creator uploads, QC review, publication authority and public listening permissions.

## Product invariants to verify

- Update one chapter: unrelated audio assets/release versions are unchanged.
- Replay the same finalize request: one immutable accepted asset, no double release.
- Upload wrong digest/object/episode: reject without making bytes public.
- Expired grant or cross-owner submission: reject regardless of a valid checksum.
- Producer times out: Cantos does not attempt a TTS retry or record a fictitious vendor charge.
- Editor rejects QC: no public release; previous published version stays playable.
- Repeated play: only streams the approved rendition, with no synthesis cost.
