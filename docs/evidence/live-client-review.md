# Independent client boundary review

Read-only reviewer: separate agent using `cantos-code-review`. BASE `7396ede`; reviewed checkpoint
`d2d784fda70a9c1b72ae7e0903b3766a309ee388`. Assessment: **no-actionable-findings** after fixes.
No source edit, PR comment/approval, merge or heavy gate rerun occurred in the reviewer task.

| Reviewed invariant | Coverage / disposition |
| --- | --- |
| credentials and surfaces | Studio owner cookie versus producer bearer/Omit; Theatre fixed reserved group and no credential input — held |
| async acknowledgement and replay | actor/session/newer/cancel guards; unknown outcomes retain exact command; owner receipt reconciliation — held |
| immutable review subject | complete rendition/checksum/decision/evidence consent; selected-unit/file state — held |
| wire compatibility | actual PUT rendition/204revocation; shared Rust/Kotlin error fixtures — held |
| verification resource safety | fresh owned cluster only; validated quoted names/no FORCE; all five restart DBs retained — held |

Grounded findings fixed: wrong PUT DTO; confirmation reused for another rendition/evidence;
metadata/file from a prior unit; local file failure wedging pending state; login/cookie races;
retry auth denial losing the original uncertain intent; blocked reconciliation after upload bearer
revocation; changed parent retaining dependent state; missing direct Catalog restart retention.
Focused regressions and the final logs in [evidence](live-client-integration.md) cover those fixes.

Read coverage: Studio live modules/view wiring, Theatre live modules/main/view, CMP common/JVM
adapter/tests, raw fixtures, docs and package-graph selection changes, serving-context helper and
PostgreSQL runner/reaper tests. Reviewer inspected Rust/Web/repository/CMP/full-PG logs; execution
and provenance remain owned by the main task. Full changed-only preflight, Android compile and
blocked browser runtime were completed/reported afterward; no new source change is implied by
those checks. User E2E, visual/accessibility, Kani, native listening, publication/delivery and
production identity remain residual risk, not review acceptance.
