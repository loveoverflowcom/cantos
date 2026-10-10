# Story workspace skills — setup evidence

Date: 2026-10-09. Scope: four agent skills, shared editorial workspace documents,
templates and the local story-folder initializer. This is operational support for
Cantos; production workers and background synchronization are not implemented here.

## Completion report

**Invariant:** Each story retains its source identity and original acquired content;
adaptations pin an explicit, versioned mapping for every proper name; artifacts and
indexes hand off between acquisition, adaptation, audio production and audit without
silently replacing another story or claiming an unverified synchronization.

**Owner / boundary:** [Story workspace](../production/story-workspace.md) owns the
Drive/local exchange contract; [radio score](../production/radio-score.md) owns the
editorial workflow. The four skills route execution through those documents and
existing Script IR, production and publication owners. Drive holds editorial
artifacts; PostgreSQL and published media storage retain their existing authority.
The initializer separates pure identity/slug decisions from filesystem effects.

**Evidence level:** `documented`, `statically-checked`, `example-tested`,
`integration-tested` (the two explicitly identified Drive files only).

**Evidence / command:**

| Check | Exact command or operation | Result |
| --- | --- | --- |
| Repository links, JSON and skill structure | `python3 scripts/check_repository.py` | Pass; 15 skills |
| Repository Python regressions | `python3 -m unittest discover -s scripts -p 'test_*.py'` | Pass; 44 tests including 21 initializer tests |
| Patch whitespace | `git diff --check` | Pass |
| New skill validation | `python3 /home/manhpd/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/<name>` for each name below | Pass for all four |
| Independent editorial rehearsal | Original fictional “Gió Qua Đồi” fixture; source inventory, two name-map revisions, provisional offline draft and repeat initialization | Pass for the exercised examples; agent rehearsal, not an automatic semantic validator |
| Live Drive write and verification | Upload/update, metadata parent check, raw download and local SHA-256 comparison | Pass for `workflow.md` and `index.md` |

Validated skills: `cantos-story-ingest`, `cantos-radio-adapt`,
`cantos-audio-produce`, `cantos-story-audit`.

**Provenance:** Working tree based on commit
`1a0e08340af47af600fad2ae27289dfd58c45a37` on `feat/script-revisions`.
Existing backend/contract work was already uncommitted and preserved. No Rust code
was changed for this scope. The rehearsal used original fictional text, not a
downloaded manuscript. Local project artifacts are at
`/home/manhpd/Desktop/Cantos`, outside Git. No story content was imported during
this setup, and no commit or pull request was created.

Verified live artifacts in the user-specified Cantos Drive folder:

| Artifact | Remote identity | Verified SHA-256 |
| --- | --- | --- |
| [workflow.md](https://drive.google.com/file/d/1O8L2Wt5uOklmHxGrfAWvV9v2_2zFjYDE/view) | `1O8L2Wt5uOklmHxGrfAWvV9v2_2zFjYDE` | `61c179fa8b9f57a675c43b38aa65fabd414be16c33d4e4d1b8734e9ecb1f7c5b` |
| [index.md](https://drive.google.com/file/d/1Zkb9fSYXn3E85VPLR85AXH9u0xg0I6on/view) | `1Zkb9fSYXn3E85VPLR85AXH9u0xg0I6on` | `6208391b5de1d0483ed3eb00bb92159698496f9ba65ff076ebdcfaba04df9d45` |

Both have parent `185z1tU88YvQkcS6mjCQsXhxiYwebkhqJ`. Private local operation
receipts record raw-readback verification. Live synchronization succeeded, so this
setup did not need a GitHub recovery issue.

**Edge classes covered:** URL tracking versus meaningful query parameters; stable
UUID identity; conflicting metadata and slugs; malformed metadata; symlinks and
Git-worktree rejection; initialization reruns preserving existing bytes. Editorial
examples distinguish a person “Bình” from the common noun in “bình sứ”, retain a
distinct “Bình An”, separate beast/faction uses of “Hắc Vũ”, reuse place identities,
and add new names without altering the six existing entity IDs or the first map
snapshot. Offline draft rehearsal uses `binding_pending` and `not_for_tts` until
real backend references exist. Local pending-sync and issue-draft examples were
simulated; they are not a real network-outage integration test.

**Not covered / residual risk:** These skills require an agent to execute the
documented checks; they do not automatically prove complete proper-name coverage.
No actual browser acquisition, OCR run, strict Script IR ingestion, live TTS call,
mix, loudness measurement or listening review ran in this setup. No recurring sync
service, concurrent Drive reconciliation or real outage/retry recovery engine was
built or tested. Rust/database/mobile gates were outside this unchanged scope.
