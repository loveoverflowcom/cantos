# UI system publication verification

- Date: 2026-10-11
- PR: [#33](https://github.com/loveoverflowcom/cantos/pull/33), targeting `develop`
- Branch: `feat/leptos-ui-system`
- Checked application source: `e63a109ba4806fe28c07145275f57b0fc1ad082d`
- Scope: publish the requested Leptos UI sequence; preserve merged upstream work;
  no CMP implementation, backend capability expansion or production acceptance

The requester subsequently authorized pushing all UI commits and merging into develop.
This supersedes the original four-session local-only stopping boundary in the
[independent review](ui-system-review.md). GitHub owns PR execution status; the
[queue](../work-plan/015-leptos-ui-system.md) retains scope and remaining acceptance work.

## Develop reconciliation and source review

The application checkpoint contains merged PR29, PR30, PR31 and PR32. PR32 was still open
at the first publication read, then develop advanced during verification. Its merged develop
SHA `5e65a90a2779d85adf3cb92648a47baff357b396` is the second parent of the checkpoint.
PR31's local-verification policy was integrated at
`c9e1a97282d40098e78e5151c4c164114a647190`.

The PR32 merge preserves both Studio `ui` and `wire` exports. API, contract and client
transport sources are byte-identical to merged develop; `REVISION_CODEC` remains JSON.
Shared UI ownership and the MessagePack contract row coexist in the layout documentation.
MessagePack keeps ADR 0013; the same UI decision moves to [0014](../decisions/0014-ui-system-foundation.md).
The reconciled xtask retains both MessagePack contract targets and shared UI/gallery gates.

Publication review found one additional compatibility defect: the disposable mutation workspace
omitted the new UI member. Commit `0f103159ab1444395a0489bbb5855a7d5545274b` copies the shared
package. The regression independently reads the root workspace manifest and inspects the actual
copy passed to the baseline runner; it failed on the missing UI manifest before the correction
and passed afterward. It does not compile or claim to kill the nine source mutants.
No additional actionable source finding remains in this publication integration scope.

## Local execution on the combined source

Rust/cargo 1.87.0 is installed under the `stable` alias; no toolchain was downloaded.
Python 3.13.5, Trunk 0.21.14 and PostgreSQL 17.4 were used. Successful combined commands
used `RUSTUP_TOOLCHAIN=stable CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 TMPDIR=/private/tmp NO_COLOR=true`.

| Command / check | Actual result |
| --- | --- |
| `cargo xtask preflight --changed-only --skip mutation --skip postgres` | 10 PASS; mutation and postgres explicitly NOT_RUN in this command |
| `cargo xtask verify contracts` | PASS: Script IR reference, contract tests including JSON/MessagePack, WAV reference |
| `cargo xtask verify postgres` | PASS: fresh disposable cluster, 56 database tests plus live HTTP kill/restart/replay and independent MessagePack oracle |
| Built artifact inspection | PASS: canonical token/foundation CSS bytes and local asset links in Studio, Theatre and gallery |

The repository gate ran 145 Python tests; workspace native tests passed 514 with 60 ignored.
Ignored tests are not runtime evidence. PostgreSQL's selected 16 revision, 9 import,
21 adaptation and 10 production tests ran separately against the new local cluster.
Its live MessagePack head/replay matched JSON through the independent Python oracle; a 406
remained a JSON error. The harness stopped its own hosts and cluster. Existing databases and
prior fixture receipts/logs were retained; generation and cost attempts remained zero in the
production/adaptation fixture journeys.

Early attempts are retained, not relabeled: one existing ephemeral-port assertion failed under
parallel test scheduling; the serial rerun passed with unchanged assertions. Subsequent WASM
lint and Studio build attempts failed with disk exhaustion. Disabling incremental compilation
also could not link xtask before space was restored. The requester explicitly approved Cantos
compiler caches and Xcode build/cache/index cleanup. After process checks, only inactive
Cantos incremental caches were removed before verification; dependencies, source, Git,
databases, audio/model assets and evidence were retained. The complete combined preflight
rerun above then passed. The failed and successful logs, receipt copies, artifact hashes and
byte-accounted cleanup records remain outside Git in `target/ui-system-publication/`.

Mutation remains **NOT_RUN**: Script IR implementation/encoder did not change, and the bounded
UI sequence avoids a cold backend build. The workspace-copy regression is `example-tested`;
no full mutation result is claimed. GitHub CI is **not checked**; no waiting, polling,
remote rerun or branch-protection bypass is part of this publication.

## Runtime evidence and residual risk

The earlier [review](ui-system-review.md) owns actual Safari 18.6 gallery/Studio observations
at 320/360/768/1440 CSS widths, light/dark, keyboard/dialog focus and native 200% zoom.
The combined MessagePack publication source was not driven again in a browser. Final CSS and
Theatre/gallery artifacts match that earlier inspection; Studio WASM changes with the preserved
transport implementation. Build/native/HTTP evidence does not certify browser interaction.

VoiceOver, real OS Vietnamese IME, reduced motion/forced colors, full focus traversal,
other browsers/devices and mobile remain **NOT_RUN**. CMP remains documentation only;
Theatre remains a skeleton. Font rights, comprehensive accessibility and production/listening
acceptance remain separate. No private design IDs/raw HTML, credentials, manuscripts,
generated audio or database/runtime artifacts are committed.

Residual risk: bounded Safari examples plus local source/build/database checks leave the named
runtime, platform, rights and production gates open; publishing the code does not waive them.
