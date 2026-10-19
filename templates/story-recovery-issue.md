# Recover story workspace operation

Use a safe title: `Recover <step> for <story ID>`. Search the resolved GitHub repository for the
deduplication key before creating an issue; update an existing match rather than duplicating it.
This records the manual ordinary Drive/local-mirror workflow, not Forge app-data/provider recovery.
Replace the fields below with observed facts, preserving the operation ID across retries.

- Dedupe key: `story-workspace:<work-id>:<blocked-step>`
- Work ID and safe display title:
- Step: capture | identity | extract | adapt | render | audit | drive_sync
- Operation ID / UTC:
- Expected result / observed failure:
- Artifact relative path, checksum and source revision (no file contents):
- Requested coverage / completed coverage / gaps:
- Verified remote IDs or public URLs, if safe:
- Safe diagnostic / retry history / ambiguity remaining:
- Last fresh state check / known remote version / outcome still unknown:
- Locally retained output: yes | no; sync state:
- Required recovery action:
- Completion evidence: remote readback/checksum or restored source access:

After a timeout or uncertain write, reconcile destination IDs and exact bytes before a retry.
For an uncertain issue-create response, search/read back the dedupe key before creating again.
Same filename with different bytes is a conflict/new revision, not permission to overwrite.
Record the bounded retry and any remaining index handoff; never mark synced on request send.
Stop blind retries for login, permission or CAPTCHA and follow the runtime's handoff policy.

Keep credentials, cookies, signed URLs, private filesystem paths, manuscript passages and audio
out of the issue. If the issue cannot be submitted, preserve this completed body outside Git and
mark `pending_issue`; never fabricate a URL. See the
[workspace recovery procedure](../docs/production/story-workspace.md#offline-and-blocked-recovery).
