# Work-level Library projection fixtures (1.3.0 specification)

Original synthetic fixtures for the `presentation=2` Saved and Continue page reads accepted in
[ADR 0057](../../../../../../../../docs/decisions/0057-listener-library-work-projection.md). No route
serves them yet; L02/L03 make the Server produce them and L04/L05 make Web and CMP decode them.

- `saved_work.json`: a playable novel with a cover and a saved story whose published chapters
  have no audio yet (`listen: no_audio_yet`, never `null`).
- `continue_work.json`: one item per `next.kind`: a finished chapter 12 pointing to chapter 13
  (`start_next`), an exact podcast resume, a replaced release offered by explicit choice and an
  anchor whose audio is unavailable.
- `continue_work_caught_up.json`: the page of an account whose only Work is caught up; caught-up
  and waiting-for-audio Works are not listed.

The 1.2.0 corpora in the parent directory, `presentation/` and `bookmarks/` are unchanged. The
CMP generator reads only those three, so this directory adds nothing to the current Kotlin
fixture maps. The Server's schema test validates these files and the per-kind negatives.

Contract: [Library v1](../../README.md#work-level-projection-v130-adr-0057-specified-not-served).
