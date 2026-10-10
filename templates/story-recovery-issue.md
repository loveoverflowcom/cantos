# Recover story workspace operation

Use a safe title: `Recover <step> for <story ID>`. Search the resolved GitHub repository for the
deduplication key before creating an issue. Replace the fields below with observed facts.

- Dedupe key: `story-workspace:<work-id>:<blocked-step>`
- Work ID and safe display title:
- Step: capture | identity | extract | adapt | render | audit | drive_sync
- Operation ID / UTC:
- Expected result / observed failure:
- Artifact relative path, checksum and source revision (no file contents):
- Requested coverage / completed coverage / gaps:
- Verified remote IDs or public URLs, if safe:
- Safe diagnostic / retry history / ambiguity remaining:
- Locally retained output: yes | no; sync state:
- Required recovery action:
- Completion evidence: remote readback/checksum or restored source access:

Keep credentials, cookies, signed URLs, private filesystem paths, manuscript passages and audio
out of the issue. If the issue cannot be submitted, preserve this completed body outside Git and
mark `pending_issue`; never fabricate a URL. See the
[workspace recovery procedure](../docs/production/story-workspace.md#offline-and-blocked-recovery).
