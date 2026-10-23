# Audio Lifecycle v1 raw fixture

`prepare-recording.json` is synthetic transport only. UUIDs/digests/credential are placeholders, not persisted Content/Brief authority or an actual upload. Domain and real PostgreSQL/media tests own admission evidence. No audio is committed.

`rendition-response.json` is the synthetic `AudioRenditionDocument` returned by producer PUT,
not the owner `AudioSubmissionResponse`. The Studio decoder runs this raw DTO fixture.
No digest in this raw transport fixture attests to stored media or domain admission.
